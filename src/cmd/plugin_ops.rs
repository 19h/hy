//! Plugin operations shared by direct installs, named repositories, and MCP setup.
use super::plugin_cmd::{PluginInstallArgs, RepoCommands};
use crate::error::{Error, Result};
use crate::plugin::{self, index};
use std::path::{Path, PathBuf};

pub(super) mod source;

mod existing;
pub(super) use existing::{InstallAction, installation_action};

pub(super) fn ida_version() -> Option<String> {
    crate::config::Env::global()
        .current_ida_version
        .clone()
        .or_else(plugin::detect_current_ida_version)
}

fn installed_constraints(metadata: &plugin::PluginMetadata) -> Result<Vec<String>> {
    let mut dependencies = Vec::new();
    for installed in plugin::installed_plugins()? {
        if installed.metadata.name.eq_ignore_ascii_case(&metadata.name) {
            continue;
        }
        let Ok(requirements) =
            plugin::components::python_dependencies(&installed.metadata, &installed.path)
        else {
            continue;
        };
        for value in requirements {
            match value.parse::<pep508_rs::Requirement>() {
                Ok(mut requirement) => {
                    if !requirement.extras.is_empty() {
                        crate::util::fmt::warning(&format!(
                            "{}: removing extras from Python constraint {value}",
                            installed.metadata.name
                        ));
                        requirement.extras.clear();
                    }
                    dependencies.push(requirement.to_string());
                }
                Err(_) => crate::util::fmt::warning(&format!(
                    "{}: ignoring invalid Python constraint {value}",
                    installed.metadata.name
                )),
            }
        }
    }
    Ok(dependencies)
}

pub async fn install(args: PluginInstallArgs, context: &plugin::PluginContext) -> Result<()> {
    super::plugin_plan::install(args, context).await
}

fn extract_wheelhouse(
    bundle: &plugin::bundle::BundleReader,
    exe_version: &str,
    destination: &Path,
) -> Result<PathBuf> {
    let manifest = bundle.manifest();
    let platform = crate::ida::current_ida_platform()?;
    let Some(target) = manifest
        .target_platform_tags
        .iter()
        .find(|tag| tag.python_version == exe_version && tag.ida_platform == platform)
    else {
        let available = manifest
            .target_platform_tags
            .iter()
            .map(|target| target.id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let available = if available.is_empty() {
            "none"
        } else {
            &available
        };
        return Err(Error::PluginInstall(format!(
            "bundle has no wheelhouse for {platform} Python {exe_version}; available targets: {available}"
        )));
    };
    bundle.extract_wheelhouse(target, destination)?;
    Ok(destination.into())
}

pub(super) async fn install_local(
    mut distribution: plugin::InstallationSource,
    args: &PluginInstallArgs,
    bundle: Option<&plugin::bundle::BundleReader>,
    context: &plugin::PluginContext,
    selected_name: Option<&str>,
) -> Result<()> {
    let replace = args.force || args.upgrade || args.editable;
    let editable_path = distribution.editable_path().map(Path::to_path_buf);
    let metadata = distribution.metadata(selected_name)?;
    let was_installed = match installation_action(&metadata, args)? {
        InstallAction::Install {
            was_installed,
        } => was_installed,
        InstallAction::Keep(installed) => {
            existing::report_current(&installed);
            return Ok(());
        }
    };
    if let Some(requirement) = &metadata.requires_python {
        let version = crate::ida::python::full_version().await?;
        if !version
            .as_deref()
            .is_some_and(|version| plugin::requirements::python_matches(version, requirement))
        {
            return Err(Error::PluginInstall(format!(
                "{} requires Python {requirement}; detected {}",
                metadata.name,
                version.as_deref().unwrap_or("unknown")
            )));
        }
    }
    let prepared = distribution.prepare(ida_version().as_deref(), replace, &metadata)?;
    let proposed = prepared.dependencies()?;
    let dependencies = if proposed.is_empty() {
        None
    } else {
        Some(installed_constraints(&metadata)?)
    };
    let wheelhouse_temp = tempfile::tempdir()?;
    let bundle = bundle.filter(|_| !context.pip.has_custom_sources());
    let resolved = if !proposed.is_empty() || bundle.is_some() {
        Some(crate::ida::python::resolve().await?)
    } else {
        None
    };
    if !proposed.is_empty() {
        crate::ida::python::validate_dependency_environment(
            resolved.as_ref().expect("dependency interpreter resolved"),
            &proposed,
            context.pip.skip_environment_check,
        )
        .await?;
    }
    let wheelhouse = match bundle {
        Some(bundle) => Some(extract_wheelhouse(
            bundle,
            &crate::ida::python::inspect(
                &resolved.as_ref().expect("bundle interpreter resolved").exe,
            )
            .await?
            .version,
            wheelhouse_temp.path(),
        )?),
        None => None,
    };
    let registration = plugin::EditableRegistration::prepare(
        &metadata.name,
        editable_path.as_deref(),
        resolved.as_ref().map(|python| python.exe.as_path()),
    )
    .await?;
    if let Some(constraints) = dependencies {
        let resolved = resolved.as_ref().expect("dependency interpreter resolved");
        let mut pip = context.pip.clone();
        pip.no_build_isolation |= args.no_build_isolation;
        crate::ida::python::verify_with_constraints(
            &resolved.exe,
            &proposed,
            &constraints,
            &pip,
            wheelhouse.as_deref(),
        )
        .await
        .map_err(|error| match error {
            Error::PythonPackages(reason) => {
                let mut message =
                    format!("Cannot install required Python dependencies: {}", proposed.join(", "));
                if !reason.is_empty() {
                    message.push_str(&format!(". Reason: {reason}"));
                }
                Error::PluginInstall(message)
            }
            other => other,
        })?;
        crate::ida::python::install_with_constraints(
            &resolved.exe,
            &proposed,
            &constraints,
            &pip,
            wheelhouse.as_deref(),
        )
        .await?;
    }
    let target = prepared.publish(registration)?;
    let configuration =
        plugin::components::metadata_tree(&metadata).into_iter().try_for_each(|component| {
            let arguments: Vec<_> = args
                .config
                .iter()
                .filter_map(|argument| {
                    let (key, value) = argument.split_once('=')?;
                    match key.split_once('.') {
                        Some((name, key)) if name.eq_ignore_ascii_case(&component.name) => {
                            Some(format!("{key}={value}"))
                        }
                        Some((name, _))
                            if plugin::components::metadata_tree(&metadata)
                                .iter()
                                .any(|component| component.name.eq_ignore_ascii_case(name)) =>
                        {
                            None
                        }
                        _ if component.name == metadata.name => Some(argument.clone()),
                        _ => None,
                    }
                })
                .collect();
            super::plugin_settings::for_install(component, &arguments)
                .and_then(|values| plugin::set_plugin_settings(&component.name, &values))
        });
    if let Err(error) = configuration {
        if !was_installed {
            plugin::uninstall(&metadata.name).await.map_err(|cleanup| {
                Error::PluginInstall(format!(
                    "{error}; failed to remove the unconfigured installation: {cleanup}"
                ))
            })?;
        }
        return Err(error);
    }
    crate::util::fmt::success(&format!(
        "Installed {}=={} at {}",
        metadata.name,
        metadata.version,
        target.display()
    ));
    Ok(())
}

pub async fn repo(command: RepoCommands, context: &plugin::PluginContext) -> Result<()> {
    let (mut repos, mut default) = index::repositories()?;
    match command {
        RepoCommands::List => {
            for (name, repo) in repos {
                println!(
                    "{name}\t{}\t{}",
                    repo.url,
                    if name == default {
                        "default"
                    } else {
                        ""
                    }
                );
            }
        }
        RepoCommands::Add(args) => {
            if !index::valid_repo_name(&args.name) {
                return Err(Error::Other("repository name must match [a-z0-9-]+".into()));
            }
            let url = url::Url::parse(&args.url).map_err(|e| Error::Other(e.to_string()))?;
            if !matches!(url.scheme(), "https" | "file") {
                return Err(Error::Other("repository URL must use https:// or file://".into()));
            }
            if index::RESERVED.iter().any(|(name, url)| *name == args.name && *url != args.url) {
                return Err(Error::Other("reserved repository URL cannot be changed".into()));
            }
            repos.insert(
                args.name,
                index::Repository {
                    url: args.url,
                },
            );
            index::save_repositories(&repos, &default)?;
        }
        RepoCommands::Remove(args) => {
            if repos.remove(&args.name).is_none() {
                return Err(Error::NotFound(args.name));
            }
            if default == args.name {
                default = if repos.contains_key("community") {
                    "community".into()
                } else {
                    repos.keys().next().cloned().unwrap_or_else(|| "community".into())
                };
            }
            index::save_repositories(&repos, &default)?;
        }
        RepoCommands::SetDefault(args) => {
            if !repos.contains_key(&args.name) {
                return Err(Error::NotFound(args.name));
            }
            index::save_repositories(&repos, &args.name)?;
        }
        RepoCommands::Snapshot => {
            let mut snapshot = index::Snapshot {
                version: 1,
                plugins: Vec::new(),
            };
            for (name, repository) in context.repositories()? {
                let loaded =
                    match context.load_repository(name.as_deref(), &repository.url, false).await {
                        Ok(loaded) => loaded,
                        Err(error) if context.repository.is_some() => return Err(error),
                        Err(error) => {
                            crate::util::fmt::warning(&format!(
                                "skipping plugin repository {}: {error}",
                                name.as_deref().unwrap_or("repository")
                            ));
                            continue;
                        }
                    };
                for note in loaded.notes {
                    crate::util::fmt::warning(&note);
                }
                snapshot.plugins.extend(loaded.snapshot.plugins);
            }
            println!("{}", snapshot.to_json()?);
        }
    }
    Ok(())
}
