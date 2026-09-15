//! Prepare plugin files, publish replacements, and remove installations.

use std::io::{Read, Seek};
use std::path::{Path, PathBuf};

use super::{
    PluginMetadata, installed_plugin_path, is_ida_version_compatible, is_platform_compatible,
    plugins_dir, read_installed_metadata, validate_name,
};
use crate::error::{Error, Result};

mod archive;
mod directory;
mod source;
pub use source::InstallationSource;

/// Validate that a plugin can be installed.
fn validate_can_install(metadata: &PluginMetadata, ida_version: Option<&str>) -> Result<()> {
    validate_metadata(metadata, ida_version)?;

    match installed_plugin_path(&metadata.name) {
        Ok(_) => return Err(Error::PluginAlreadyInstalled(metadata.name.clone())),
        Err(Error::PluginNotInstalled(_)) => {}
        Err(error) => return Err(error),
    }

    Ok(())
}

fn validate_metadata(metadata: &PluginMetadata, ida_version: Option<&str>) -> Result<()> {
    validate_name(&metadata.name)?;
    if crate::plugin::parse_version(&metadata.version).is_none() {
        return Err(Error::PluginInstall(format!("invalid plugin version: {}", metadata.version)));
    }
    if !is_platform_compatible(metadata, &crate::ida::current_ida_platform()?) {
        return Err(Error::PlatformIncompatible(format!(
            "Plugin '{}' is not compatible with this platform",
            metadata.name
        )));
    }

    // IDA version check.
    if let Some(ver) = ida_version
        && !is_ida_version_compatible(metadata, ver)
    {
        return Err(Error::IdaVersionIncompatible(format!(
            "Plugin '{}' is not compatible with IDA {}",
            metadata.name, ver
        )));
    }

    Ok(())
}

/// Validated plugin files whose publication is delayed until dependencies are ready.
pub struct PreparedPlugin {
    metadata: PluginMetadata,
    staging: tempfile::TempDir,
    path: PathBuf,
    replace: bool,
}

impl PreparedPlugin {
    pub fn dependencies(&self) -> Result<Vec<String>> {
        super::dependencies_from_directory(&self.metadata, &self.path)
    }

    pub fn publish(self, registration: super::EditableRegistration) -> Result<PathBuf> {
        commit_staged(self.staging, &self.path, &self.metadata.name, self.replace, registration)
    }
}

fn prepare_archive<R: Read + Seek>(
    archive: &mut crate::util::python_zip::Archive<R>,
    ida_version: Option<&str>,
    force: bool,
    name: Option<&str>,
) -> Result<PreparedPlugin> {
    let selected = crate::plugin::select_archived_plugin(archive, name)?;
    let metadata = selected.metadata;
    let root = selected.prefix;
    super::files::ArchiveReferences::read(archive).validate(&metadata, &root)?;
    validate_metadata(&metadata, ida_version)?;
    if !force {
        validate_can_install(&metadata, ida_version)?;
    }

    let staging = staging_directory()?;
    let target_dir = staging.path().join("new");
    archive::extract(archive, &root, &target_dir)?;

    super::validate_directory_files(&metadata, &target_dir)?;
    Ok(PreparedPlugin {
        metadata,
        staging,
        path: target_dir,
        replace: force,
    })
}

/// Install a plugin editable: symlink `$IDAUSR/plugins/<name>` to the
/// source directory so edits take effect on the next plugin reload.
fn prepare_editable(
    source_dir: &Path,
    ida_version: Option<&str>,
    force: bool,
) -> Result<PreparedPlugin> {
    let source_dir = source_dir
        .canonicalize()
        .map_err(|e| Error::PluginInstall(format!("cannot resolve source directory: {e}")))?;
    let metadata = crate::plugin::read_metadata_from_directory(&source_dir)?;
    validate_metadata(&metadata, ida_version)?;
    super::validate_directory_files(&metadata, &source_dir)?;
    if !force {
        validate_can_install(&metadata, ida_version)?;
    }

    let staging = staging_directory()?;
    let target = staging.path().join("new");
    let installed_path = match installed_plugin_path(&metadata.name) {
        Ok(path) => path,
        Err(Error::PluginNotInstalled(_)) => plugins_dir().join(&metadata.name),
        Err(error) => return Err(error),
    };
    if !installed_path.is_symlink()
        && installed_path.canonicalize().is_ok_and(|path| source_dir.starts_with(path))
    {
        return Err(Error::PluginInstall(
            "editable source is inside the installation being replaced".into(),
        ));
    }

    #[cfg(unix)]
    std::os::unix::fs::symlink(&source_dir, &target)
        .map_err(|e| Error::PluginInstall(format!("symlink failed: {e}")))?;
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(&source_dir, &target).map_err(|e| {
        Error::PluginInstall(format!(
            "symlink failed: {e} (developer mode or admin rights may be required)"
        ))
    })?;

    Ok(PreparedPlugin {
        metadata,
        staging,
        path: target,
        replace: force,
    })
}

fn staging_directory() -> Result<tempfile::TempDir> {
    std::fs::create_dir_all(plugins_dir())?;
    let actual = plugins_dir().canonicalize()?;
    let parent = actual
        .parent()
        .ok_or_else(|| Error::PluginInstall("plugins directory has no parent".into()))?;
    Ok(tempfile::Builder::new().prefix(".hy-plugin-").tempdir_in(parent)?)
}

/// Publish by same-filesystem rename. Preserve the original until the replacement is complete.
fn commit_staged(
    staging: tempfile::TempDir,
    staged: &Path,
    name: &str,
    replace: bool,
    registration: super::EditableRegistration,
) -> Result<PathBuf> {
    validate_name(name)?;
    let original = match installed_plugin_path(name) {
        Ok(path) => Some(path),
        Err(Error::PluginNotInstalled(_)) => None,
        Err(error) => return Err(error),
    };
    // The new directory must match its descriptor exactly, including case.
    // Moving the old entry out first also permits case-only renames on APFS/NTFS.
    let target = plugins_dir().join(name);
    let backup = staged.with_file_name("previous");
    if let Some(original) = &original {
        if !replace {
            return Err(Error::PluginAlreadyInstalled(name.into()));
        }
        std::fs::rename(original, &backup)?;
    }
    if let Err(error) = std::fs::rename(staged, &target) {
        if let Some(original) = &original
            && let Err(rollback) = std::fs::rename(&backup, original)
        {
            // Retain the owner before returning: its destructor would delete the backup.
            let _retained = staging.keep();
            return Err(Error::PluginInstall(format!(
                "publish failed: {error}; rollback failed: {rollback}; original retained at {}",
                backup.display()
            )));
        }
        return Err(error.into());
    }
    if let Err(error) = registration.publish() {
        let rollback = std::fs::rename(&target, staged).and_then(|()| {
            if let Some(original) = &original {
                std::fs::rename(&backup, original)
            } else {
                Ok(())
            }
        });
        if let Err(rollback) = rollback {
            let retained = staging.keep();
            return Err(Error::PluginInstall(format!(
                "editable registration failed: {error}; rollback failed: {rollback}; staging retained at {}",
                retained.display()
            )));
        }
        return Err(error);
    }
    Ok(target)
}

/// Remove a plugin directory; editable installs only remove the symlink,
/// never the source tree it points to.
fn remove_plugin_dir(dir: &Path) -> Result<()> {
    let meta = std::fs::symlink_metadata(dir)?;
    if meta.file_type().is_symlink() {
        #[cfg(unix)]
        std::fs::remove_file(dir)?;
        #[cfg(windows)]
        std::fs::remove_dir(dir)?;
    } else if meta.is_dir() {
        std::fs::remove_dir_all(dir)?;
    } else {
        std::fs::remove_file(dir)?;
    }
    Ok(())
}

/// Uninstall a plugin by name.
pub async fn uninstall(name: &str) -> Result<()> {
    let dir = installed_plugin_path(name)?;
    let editable = dir.is_symlink();
    let canonical_name = read_installed_metadata(name)
        .ok()
        .map(|metadata| metadata.name)
        .unwrap_or_else(|| dir.file_name().unwrap().to_string_lossy().into_owned());
    let staging = staging_directory()?;
    let removed = staging.path().join("removed");
    std::fs::rename(&dir, &removed)?;
    // The plugin is logically removed; a deletion error must not destroy the original piecemeal.
    if let Err(e) = remove_plugin_dir(&removed) {
        let retained = staging.keep();
        tracing::warn!("removed plugin retained at {}: {e}", retained.display());
    }
    if editable {
        super::EditableRegistration::prepare(&canonical_name, None, None).await?.publish()?;
    }
    Ok(())
}
