//! Plugin operations shared by direct installs, named repositories, and MCP setup.
use super::plugin_cmd::{PluginInstallArgs, RepoCommands};
use crate::error::{Error, Result};
use crate::plugin::{self, index};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum UpgradePolicy {
    KeepCurrentIfNewer,
    RequireNewer,
}

pub(super) fn ida_version() -> Option<String> {
    crate::config::Env::global()
        .current_ida_version
        .clone()
        .or_else(plugin::detect_current_ida_version)
}

fn combined_dependencies(
    metadata: &plugin::PluginMetadata,
    proposed: Vec<String>,
) -> Result<Vec<String>> {
    let mut dependencies = Vec::new();
    for installed in plugin::installed_plugins()? {
        if installed.metadata.name.eq_ignore_ascii_case(&metadata.name) {
            continue;
        }
        dependencies
            .extend(plugin::dependencies_from_directory(&installed.metadata, &installed.path)?);
    }
    dependencies.extend(proposed);
    Ok(dependencies)
}

pub async fn install(args: PluginInstallArgs, context: &plugin::PluginContext) -> Result<()> {
    if args.editable && !Path::new(&args.source).is_dir() {
        return Err(Error::PluginInstall("--editable requires a local directory".into()));
    }
    let temporary = tempfile::tempdir()?;
    let mut source = PathBuf::from(&args.source);
    let mut loaded = None;
    let mut selected_name = None;
    if !source.exists() {
        let bytes = if args.source.starts_with("https://github.com/")
            && !args.source.contains("/releases/download/")
        {
            index::github_archive(&args.source).await?
        } else if args.source.starts_with("https://")
            || args.source.starts_with("http://")
            || args.source.starts_with("file:")
        {
            index::fetch(&args.source).await?
        } else {
            let reference = index::parse_reference(&args.source)?;
            let repository = loaded.insert(context.load_for(&reference).await?);
            let location =
                index::select(&repository.snapshot, &reference, ida_version().as_deref())?;
            selected_name = Some(location.descriptor.metadata.name.clone());
            repository.fetch_verified(location).await?
        };
        source = temporary.path().join("plugin.zip");
        std::fs::write(&source, bytes)?;
    }
    if plugin::bundle::is_plugin_bundle_zip(&source) {
        let bundle = index::load(&source.to_string_lossy(), true).await?;
        for (index, entry) in bundle.snapshot.plugins.iter().enumerate() {
            let reference = index::Reference {
                name: entry.name.clone(),
                spec: String::new(),
                host: None,
                repo: None,
            };
            let location = index::select(&bundle.snapshot, &reference, ida_version().as_deref())?;
            let path = temporary.path().join(format!("bundle-plugin-{index}.zip"));
            std::fs::write(&path, bundle.fetch_verified(location).await?)?;
            install_local(
                &path,
                &args,
                bundle.bundle_reader(),
                context,
                Some(&location.descriptor.metadata.name),
                UpgradePolicy::KeepCurrentIfNewer,
            )
            .await?;
        }
    } else {
        install_local(
            &source,
            &args,
            loaded.as_ref().and_then(index::LoadedRepository::bundle_reader),
            context,
            selected_name.as_deref(),
            UpgradePolicy::KeepCurrentIfNewer,
        )
        .await?;
    }
    Ok(())
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
    source: &Path,
    args: &PluginInstallArgs,
    bundle: Option<&plugin::bundle::BundleReader>,
    context: &plugin::PluginContext,
    selected_name: Option<&str>,
    upgrade_policy: UpgradePolicy,
) -> Result<()> {
    let replace = args.force || args.upgrade || args.editable;
    let metadata = if source.is_dir() {
        plugin::read_metadata_from_directory(source)?
    } else {
        let mut archive = crate::util::python_zip::Archive::new(std::fs::File::open(source)?)?;
        plugin::select_archived_plugin(&mut archive, selected_name)?.metadata
    };
    let candidate_version = plugin::parse_version(&metadata.version).ok_or_else(|| {
        Error::PluginInstall(format!("invalid plugin version: {}", metadata.version))
    })?;
    let installed = match plugin::installed_plugin_path(&metadata.name) {
        Ok(path) => match plugin::read_installed_metadata_from_path(&path) {
            Ok(metadata) => Some(metadata),
            Err(error) if args.editable => {
                tracing::debug!(%error, "replacing broken installation with editable source");
                None
            }
            Err(error) => {
                return Err(Error::PluginInstall(format!(
                    "existing plugin {} is invalid: {error}; remove it with `hy plugin uninstall {}`",
                    metadata.name, metadata.name
                )));
            }
        },
        Err(Error::PluginNotInstalled(_)) => None,
        Err(error) => return Err(error),
    };
    let was_installed = installed.is_some();
    if let Some(old) = installed {
        let installed_version = plugin::parse_version(&old.version).ok_or_else(|| {
            Error::PluginInstall(format!("invalid installed plugin version: {}", old.version))
        })?;
        if old.normalized_host()? != metadata.normalized_host()? {
            return Err(Error::PluginInstall(
                "replacement belongs to a different plugin repository".into(),
            ));
        }
        if args.upgrade
            && !args.editable
            && !candidate_version.cmp_precedence(&installed_version).is_gt()
        {
            if upgrade_policy == UpgradePolicy::RequireNewer {
                return Err(Error::PluginInstall(format!(
                    "{}=={} is not newer than installed version {}",
                    metadata.name, metadata.version, old.version,
                )));
            }
            crate::util::fmt::success(&format!("Already installed {}=={}", old.name, old.version));
            return Ok(());
        }
    } else if upgrade_policy == UpgradePolicy::RequireNewer {
        return Err(Error::PluginNotInstalled(metadata.name));
    }
    let prepared = plugin::PreparedPlugin::prepare(
        source,
        ida_version().as_deref(),
        replace,
        args.editable,
        Some(&metadata),
    )?;
    let proposed = prepared.dependencies()?;
    let dependencies = if proposed.is_empty() {
        None
    } else {
        Some(combined_dependencies(&metadata, proposed.clone())?)
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
        args.editable.then_some(source),
        resolved.as_ref().map(|python| python.exe.as_path()),
    )
    .await?;
    if let Some(dependencies) = dependencies {
        let resolved = resolved.as_ref().expect("dependency interpreter resolved");
        let mut pip = context.pip.clone();
        pip.no_build_isolation |= args.no_build_isolation;
        crate::ida::python::verify_dependencies(
            &resolved.exe,
            &dependencies,
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
        crate::ida::python::install_dependencies(
            &resolved.exe,
            &dependencies,
            &pip,
            wheelhouse.as_deref(),
        )
        .await?;
    }
    let target = prepared.publish(registration)?;
    let configuration = super::plugin_settings::for_install(&metadata, &args.config)
        .and_then(|values| plugin::set_plugin_settings(&metadata.name, &values));
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
                plugins: vec![],
            };
            for (name, repo) in context.repositories()? {
                snapshot.plugins.extend(
                    context
                        .load_repository(name.as_deref(), &repo.url, false)
                        .await?
                        .snapshot
                        .plugins,
                );
            }
            println!("{}", snapshot.to_json()?);
        }
    }
    Ok(())
}
