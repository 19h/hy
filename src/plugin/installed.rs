//! Installed plugin discovery, names, and descriptor lookup.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::ida::ida_user_dir;

use super::PluginMetadata;

/// Get the plugins directory inside `$IDAUSR`.
pub fn plugins_dir() -> PathBuf {
    ida_user_dir().join("plugins")
}

/// Resolve a filesystem entry, including broken installations and dangling links.
/// Managed-plugin callers must use the validated records instead.
pub fn installed_plugin_path(name: &str) -> Result<PathBuf> {
    if name.is_empty()
        || matches!(name, "." | "..")
        || name.contains(['/', '\\', '\t', '\r', '\n', '\0'])
    {
        return Err(Error::InvalidPluginName(name.into()));
    }
    let root = plugins_dir();
    let entries = match std::fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(Error::PluginNotInstalled(name.into()));
        }
        Err(error) => return Err(error.into()),
    };
    let wanted = name.to_lowercase();
    let mut found = None;
    for entry in entries {
        let entry = entry?;
        if entry.file_name().to_str().is_some_and(|name| name.to_lowercase() == wanted) {
            if found.is_some() {
                return Err(Error::PluginInstall(format!("multiple installed paths match {name}")));
            }
            found = Some(entry.path());
        }
    }
    found.ok_or_else(|| Error::PluginNotInstalled(name.into()))
}

/// A managed plugin with validated metadata, referenced files, and directory name.
#[derive(Debug, Clone)]
pub struct InstalledPlugin {
    pub path: PathBuf,
    pub metadata: PluginMetadata,
    /// True when the plugin directory is a symlink (editable install).
    pub editable: bool,
}

/// Enumerate the canonical records used by dependency, search, and upgrade operations.
pub fn installed_plugins() -> Result<Vec<InstalledPlugin>> {
    let dir = plugins_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut plugins = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        // Resolve symlinks (editable installs) when checking for a directory.
        if !entry.path().is_dir() {
            continue;
        }
        let path = entry.path();
        let metadata = match read_installed_metadata_from_path(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                tracing::debug!(path = %path.display(), %error, "ignoring invalid installation");
                continue;
            }
        };
        plugins.push(InstalledPlugin {
            editable: path.is_symlink(),
            path,
            metadata,
        });
    }
    plugins.sort_by(|a, b| a.metadata.name.cmp(&b.metadata.name));
    Ok(plugins)
}

pub(crate) fn read_installed_metadata_from_path(path: &Path) -> Result<PluginMetadata> {
    let metadata = super::read_metadata_from_directory(path)?;
    if path.file_name().and_then(|name| name.to_str()) != Some(metadata.name.as_str()) {
        return Err(Error::PluginInstall(format!(
            "plugin name {} does not match directory {}",
            metadata.name,
            path.display()
        )));
    }
    Ok(metadata)
}

pub fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        || name.starts_with(['_', '-'])
        || name.ends_with(['_', '-'])
    {
        return Err(Error::InvalidPluginName(name.into()));
    }
    Ok(())
}

/// Read plugin metadata from an installed plugin directory.
pub fn read_installed_metadata(name: &str) -> Result<PluginMetadata> {
    let mut matches = installed_plugins()?
        .into_iter()
        .filter(|record| record.metadata.name.eq_ignore_ascii_case(name));
    let record = matches.next().ok_or_else(|| Error::PluginNotInstalled(name.into()))?;
    if matches.next().is_some() {
        return Err(Error::PluginInstall(format!("multiple installed plugins match {name}")));
    }
    Ok(record.metadata)
}

/// Detect the IDA version from the current install directory.
pub fn detect_current_ida_version() -> Option<String> {
    let install_dir = crate::ida::current_install_dir()?;
    crate::ida::detect_ida_version(&install_dir)
}
