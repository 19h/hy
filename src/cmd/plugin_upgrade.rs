//! Resolve upgrades against the installed identity, excluding old versions before probing Python.

use std::collections::HashMap;
use std::path::Path;

use super::plugin_cmd::{PluginInstallArgs, PluginUpgradeArgs};
use super::plugin_plan;
use crate::error::{Error, Result};
use crate::plugin::repositories::Repositories;
use crate::plugin::requirements::Requirement;
use crate::plugin::{self, PluginContext, index, resolve};

async fn upgrade_one(
    source: &str,
    no_build_isolation: bool,
    context: &PluginContext,
) -> Result<()> {
    if Path::new(source).exists() || source.contains("://") && !source.contains('@') {
        return Err(Error::PluginInstall("plugin upgrade requires a repository reference; use plugin install --upgrade for local files or URLs".into()));
    }
    let reference = index::parse_reference(source)?;
    let installed = plugin::read_installed_metadata(&reference.name)?;
    let host = installed.normalized_host()?;
    if reference.host.as_ref().is_some_and(|requested| requested != &host) {
        return Err(Error::PluginInstall(format!(
            "installed plugin {} belongs to a different repository",
            installed.name
        )));
    }
    let mut scopes = HashMap::new();
    if let Some(repo) = reference.repo {
        scopes.insert(installed.name.to_lowercase(), repo);
    }
    let mut pool = Repositories::load(context, &scopes, None).await?;
    let requirement = Requirement {
        name: installed.name.clone(),
        spec: reference.spec,
        host: Some(host.clone()),
        required: true,
    };
    let plugin = pool
        .snapshot
        .plugins
        .iter_mut()
        .find(|plugin| {
            plugin.name.eq_ignore_ascii_case(&installed.name)
                && index::normalize_host(&plugin.host).ok().as_ref() == Some(&host)
        })
        .ok_or_else(|| {
            Error::NotFound(format!(
                "installed plugin {} in configured repositories",
                installed.name
            ))
        })?;
    plugin.versions.retain(|version, _| {
        plugin::version_precedence(version) > plugin::version_precedence(&installed.version)
    });
    let installed_matches = requirement.matches(&installed.version);
    if !plugin.versions.keys().any(|version| requirement.matches(version)) && installed_matches {
        crate::util::fmt::success(&format!(
            "{} is already up to date ({})",
            installed.name, installed.version
        ));
        return Ok(());
    }
    let roots = [requirement];
    let cell = plugin_plan::cell(&pool.snapshot, &roots, &[]).await?;
    let plan = match resolve::resolve(
        &pool.snapshot,
        &roots,
        &cell,
        &[],
        &plugin_plan::installed(std::slice::from_ref(&installed.name))?,
    ) {
        Ok(plan) => plan,
        Err(error) if installed_matches && !error.fatal => {
            crate::util::fmt::success(&format!(
                "{} is already up to date ({})",
                installed.name, installed.version
            ));
            println!("Newer versions cannot be installed here: {error}");
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    let args = PluginInstallArgs {
        source: source.into(),
        force: false,
        upgrade: true,
        editable: false,
        no_build_isolation,
        config: Vec::new(),
    };
    plugin_plan::apply(&pool, plan, &[installed.name], &args, context).await
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
