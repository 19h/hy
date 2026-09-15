//! Resolve upgrades by installed identity, independently of the default repository.

use std::path::Path;

use super::plugin_cmd::{PluginInstallArgs, PluginUpgradeArgs};
use super::plugin_ops::{UpgradePolicy, ida_version, install_local};
use crate::error::{Error, Result};
use crate::plugin::{self, PluginContext, index};

async fn upgrade_one(
    source: &str,
    no_build_isolation: bool,
    context: &PluginContext,
) -> Result<()> {
    if Path::new(source).exists() || source.contains("://") && !source.contains('@') {
        return Err(Error::PluginInstall(
            "plugin upgrade requires a repository reference; use plugin install --upgrade for local files or URLs".into(),
        ));
    }
    let mut reference = index::parse_reference(source)?;
    let installed = plugin::read_installed_metadata(&reference.name)?;
    let installed_host = installed.normalized_host()?;
    if reference.host.as_ref().is_some_and(|host| host != &installed_host) {
        return Err(Error::PluginInstall(format!(
            "installed plugin {} belongs to a different repository",
            installed.name,
        )));
    }
    reference.name = installed.name;
    reference.host = Some(installed_host);

    let mut repositories = Vec::new();
    let mut notes = Vec::new();
    if reference.repo.is_some() || context.repository.is_some() {
        repositories.push(context.load_for(&reference).await?);
    } else {
        for (name, repository) in context.repositories()? {
            match context.load_repository(name.as_deref(), &repository.url, false).await {
                Ok(repository) => {
                    notes.extend(repository.notes.iter().cloned());
                    repositories.push(repository);
                }
                Err(error) => {
                    notes.push(format!("{}: {error}", name.as_deref().unwrap_or("repository")))
                }
            }
        }
    }
    let candidates: Vec<_> = repositories
        .iter()
        .flat_map(|repository| {
            repository
                .snapshot
                .plugins
                .iter()
                .filter(|plugin| index::matches_reference(plugin, &reference))
                .map(move |_| repository)
        })
        .collect();
    let repository = match candidates.as_slice() {
        [repository] => *repository,
        [] => {
            return Err(Error::NotFound(format!(
                "installed plugin {} in configured repositories{}",
                reference.name,
                if notes.is_empty() {
                    String::new()
                } else {
                    format!("\n{}", notes.join("\n"))
                },
            )));
        }
        _ => {
            return Err(Error::PluginInstall(format!(
                "{} is present in multiple repositories; qualify the upgrade with REPO/{}",
                reference.name, reference.name,
            )));
        }
    };
    let location = index::select(&repository.snapshot, &reference, ida_version().as_deref())?;
    let archive = repository.fetch_verified(location).await?;
    let temporary = tempfile::tempdir()?;
    let path = temporary.path().join("plugin.zip");
    std::fs::write(&path, archive)?;
    let arguments = PluginInstallArgs {
        source: source.into(),
        force: false,
        upgrade: true,
        editable: false,
        no_build_isolation,
        config: Vec::new(),
    };
    install_local(
        &path,
        &arguments,
        repository.bundle_reader(),
        context,
        Some(&location.descriptor.metadata.name),
        UpgradePolicy::RequireNewer,
    )
    .await
}

pub async fn run(args: PluginUpgradeArgs, context: &PluginContext) -> Result<()> {
    let sources = match args.source {
        Some(source) => vec![source],
        None => plugin::installed_plugins()?
            .into_iter()
            .filter(|plugin| !plugin.editable)
            .map(|plugin| plugin.metadata.name)
            .collect(),
    };
    for source in sources {
        upgrade_one(&source, args.no_build_isolation, context).await?;
    }
    Ok(())
}
