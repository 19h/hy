//! Plugin repository abstraction and plugin installation.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::ida::ida_user_dir;
use crate::plugin::metadata::{is_ida_version_compatible, is_platform_compatible, PluginMetadata};

/// Get the plugins directory inside `$IDAUSR`.
pub fn plugins_dir() -> PathBuf {
    ida_user_dir().join("plugins")
}

/// Check if a plugin is installed by name.
pub fn is_installed(name: &str) -> bool {
    plugins_dir().join(name).exists()
}

/// List installed plugins (name, version).
pub fn installed_plugins() -> Result<Vec<(String, Option<String>)>> {
    let dir = plugins_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut plugins = Vec::new();
    for entry in std::fs::read_dir(&dir)?.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let version = read_installed_version(&entry.path());
        plugins.push((name, version));
    }
    plugins.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(plugins)
}

fn read_installed_version(plugin_dir: &Path) -> Option<String> {
    let manifest_path = plugin_dir.join("ida-plugin.json");
    if !manifest_path.exists() {
        return None;
    }
    let text = std::fs::read_to_string(&manifest_path).ok()?;
    let val: serde_json::Value = serde_json::from_str(&text).ok()?;
    val.get("version")
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// Validate that a plugin can be installed.
pub fn validate_can_install(metadata: &PluginMetadata, ida_version: Option<&str>) -> Result<()> {
    // Name validation.
    if metadata.name.is_empty()
        || metadata.name.contains('/')
        || metadata.name.contains('\\')
        || metadata.name == "."
        || metadata.name == ".."
    {
        return Err(Error::InvalidPluginName(metadata.name.clone()));
    }

    // Already installed?
    if is_installed(&metadata.name) {
        return Err(Error::PluginAlreadyInstalled(metadata.name.clone()));
    }

    // Platform check.
    if !is_platform_compatible(metadata) {
        return Err(Error::PlatformIncompatible(format!(
            "Plugin '{}' is not compatible with this platform",
            metadata.name
        )));
    }

    // IDA version check.
    if let Some(ver) = ida_version {
        if !is_ida_version_compatible(metadata, ver) {
            return Err(Error::IdaVersionIncompatible(format!(
                "Plugin '{}' is not compatible with IDA {}",
                metadata.name, ver
            )));
        }
    }

    Ok(())
}

/// Install a plugin from a zip archive.
pub fn install_from_archive(
    archive_path: &Path,
    ida_version: Option<&str>,
    force: bool,
) -> Result<PathBuf> {
    let metadata = crate::plugin::metadata::read_metadata_from_archive(archive_path)?;

    if !force {
        validate_can_install(&metadata, ida_version)?;
    }

    let target_dir = plugins_dir().join(&metadata.name);
    std::fs::create_dir_all(&target_dir)?;

    // Extract archive.
    let file = std::fs::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_owned();

        // Reject dangerous paths.
        if name.contains("..") || name.starts_with('/') || name.starts_with('\\') {
            continue;
        }

        // Skip symlinks.
        if entry.is_symlink() {
            continue;
        }

        let out_path = target_dir.join(&name);

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out_file = std::fs::File::create(&out_path)?;
            std::io::copy(&mut entry, &mut out_file)?;
        }
    }

    Ok(target_dir)
}

/// Uninstall a plugin by name.
pub fn uninstall(name: &str) -> Result<()> {
    let dir = plugins_dir().join(name);
    if !dir.exists() {
        return Err(Error::PluginNotInstalled(name.into()));
    }
    std::fs::remove_dir_all(&dir)?;
    Ok(())
}

/// Recursively copy a directory.
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Upgrade a plugin: install new version with rollback on failure.
pub fn upgrade_from_archive(archive_path: &Path, ida_version: Option<&str>) -> Result<PathBuf> {
    let metadata = crate::plugin::metadata::read_metadata_from_archive(archive_path)?;
    let target_dir = plugins_dir().join(&metadata.name);

    if !target_dir.exists() {
        return Err(Error::PluginNotInstalled(metadata.name.clone()));
    }

    // Create rollback copy.
    let rollback_dir = target_dir.with_extension("rollback");
    if rollback_dir.exists() {
        std::fs::remove_dir_all(&rollback_dir)?;
    }
    copy_dir_recursive(&target_dir, &rollback_dir)
        .map_err(|e| Error::PluginInstall(format!("rollback copy failed: {e}")))?;

    // Remove existing and install new.
    std::fs::remove_dir_all(&target_dir)?;
    match install_from_archive(archive_path, ida_version, true) {
        Ok(path) => {
            // Success — remove rollback.
            let _ = std::fs::remove_dir_all(&rollback_dir);
            Ok(path)
        }
        Err(e) => {
            // Failure — restore from rollback.
            let _ = std::fs::remove_dir_all(&target_dir);
            let _ = std::fs::rename(&rollback_dir, &target_dir);
            Err(e)
        }
    }
}
