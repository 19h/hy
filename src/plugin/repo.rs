//! Plugin repository abstraction and plugin installation.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::ida::ida_user_dir;
use crate::plugin::metadata::{is_ida_version_compatible, is_platform_compatible, PluginMetadata};

/// Get the plugins directory inside `$IDAUSR`.
pub fn plugins_dir() -> PathBuf {
    ida_user_dir().join("plugins")
}

/// Check if a plugin is installed by name (including editable symlinks,
/// even dangling ones).
pub fn is_installed(name: &str) -> bool {
    let path = plugins_dir().join(name);
    path.exists() || std::fs::symlink_metadata(&path).is_ok()
}

/// An installed plugin as found on disk.
#[derive(Debug, Clone)]
pub struct InstalledPlugin {
    pub name: String,
    pub version: Option<String>,
    /// True when the plugin directory is a symlink (editable install).
    pub editable: bool,
}

/// List installed plugins.
pub fn installed_plugins() -> Result<Vec<InstalledPlugin>> {
    let dir = plugins_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut plugins = Vec::new();
    for entry in std::fs::read_dir(&dir)?.flatten() {
        // Resolve symlinks (editable installs) when checking for a directory.
        if !entry.path().is_dir() {
            continue;
        }
        let editable = std::fs::symlink_metadata(entry.path())
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false);
        let name = entry.file_name().to_string_lossy().to_string();
        let version = read_installed_version(&entry.path());
        plugins.push(InstalledPlugin {
            name,
            version,
            editable,
        });
    }
    plugins.sort_by(|a, b| a.name.cmp(&b.name));
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
    if let Some(ver) = ida_version
        && !is_ida_version_compatible(metadata, ver) {
            return Err(Error::IdaVersionIncompatible(format!(
                "Plugin '{}' is not compatible with IDA {}",
                metadata.name, ver
            )));
        }

    Ok(())
}

/// Safely resolve an archive entry name below `base`, rejecting absolute
/// paths and any path-traversal components. Returns `None` for unsafe paths.
fn safe_join(base: &Path, entry_name: &str) -> Option<PathBuf> {
    use std::path::Component;

    let rel = Path::new(entry_name);
    let mut out = base.to_path_buf();
    for component in rel.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            // Absolute paths, drive prefixes, and `..` are all rejected
            // outright rather than skipped component-wise, so an attacker
            // can't construct a path that escapes the plugin directory.
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    // Defense in depth: the result must remain below the base directory.
    out.starts_with(base).then_some(out)
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

    // Validate every entry path before extracting anything, so a malicious
    // archive cannot leave a partial installation behind.
    let file = std::fs::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    let target_dir = plugins_dir().join(&metadata.name);
    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;
        if entry.is_symlink() {
            continue;
        }
        if safe_join(&target_dir, entry.name()).is_none() {
            return Err(Error::PluginInstall(format!(
                "archive contains an unsafe path: {}",
                entry.name()
            )));
        }
    }

    std::fs::create_dir_all(&target_dir)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_owned();

        // Skip symlinks.
        if entry.is_symlink() {
            continue;
        }

        let Some(out_path) = safe_join(&target_dir, &name) else {
            continue; // unreachable: validated above
        };

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

/// Install a plugin from a local source directory by copying it into the
/// plugins directory. The directory must contain `ida-plugin.json`.
pub fn install_from_directory(
    source_dir: &Path,
    ida_version: Option<&str>,
    force: bool,
) -> Result<PathBuf> {
    let metadata = crate::plugin::metadata::read_metadata_from_directory(source_dir)?;

    if !force {
        validate_can_install(&metadata, ida_version)?;
    }

    let target_dir = plugins_dir().join(&metadata.name);
    if force && (target_dir.exists() || std::fs::symlink_metadata(&target_dir).is_ok()) {
        remove_plugin_dir(&target_dir)?;
    }

    copy_plugin_tree(source_dir, &target_dir)
        .map_err(|e| Error::PluginInstall(format!("copy failed: {e}")))?;
    Ok(target_dir)
}

/// Copy a plugin source tree, skipping VCS and build artifacts.
fn copy_plugin_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
    const SKIP: &[&str] = &[".git", ".hg", ".svn", "__pycache__", ".venv", "venv", ".idea"];
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        if SKIP.contains(&name.to_string_lossy().as_ref()) {
            continue;
        }
        let src_path = entry.path();
        let dst_path = dst.join(&name);
        if src_path.is_dir() {
            copy_plugin_tree(&src_path, &dst_path)?;
        } else if src_path.is_file() {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Install a plugin editable: symlink `$IDAUSR/plugins/<name>` to the
/// source directory so edits take effect on the next plugin reload.
pub fn install_editable(
    source_dir: &Path,
    ida_version: Option<&str>,
    force: bool,
) -> Result<PathBuf> {
    let source_dir = source_dir
        .canonicalize()
        .map_err(|e| Error::PluginInstall(format!("cannot resolve source directory: {e}")))?;
    let metadata = crate::plugin::metadata::read_metadata_from_directory(&source_dir)?;

    if !force {
        validate_can_install(&metadata, ida_version)?;
    }

    let target = plugins_dir().join(&metadata.name);
    if force && (target.exists() || std::fs::symlink_metadata(&target).is_ok()) {
        remove_plugin_dir(&target)?;
    }
    std::fs::create_dir_all(plugins_dir())?;

    #[cfg(unix)]
    std::os::unix::fs::symlink(&source_dir, &target)
        .map_err(|e| Error::PluginInstall(format!("symlink failed: {e}")))?;
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(&source_dir, &target).map_err(|e| {
        Error::PluginInstall(format!(
            "symlink failed: {e} (developer mode or admin rights may be required)"
        ))
    })?;

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
    } else {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}

/// Uninstall a plugin by name.
pub fn uninstall(name: &str) -> Result<()> {
    let dir = plugins_dir().join(name);
    if !dir.exists() && std::fs::symlink_metadata(&dir).is_err() {
        return Err(Error::PluginNotInstalled(name.into()));
    }
    remove_plugin_dir(&dir)
}

// ── plugin settings (ida-config.json) ───────────────────────────────────

/// Read the IDA config file (`ida-config.json` in the IDA user dir).
fn read_ida_config() -> serde_json::Value {
    let path = ida_user_dir().join("ida-config.json");
    if !path.exists() {
        return serde_json::json!({});
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::json!({}))
}

/// Write the IDA config file.
fn write_ida_config(config: &serde_json::Value) -> Result<()> {
    let path = ida_user_dir().join("ida-config.json");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(config)?;
    std::fs::write(&path, text)?;
    Ok(())
}

/// Get a plugin setting from `ida-config.json` at `plugins.<name>.settings.<key>`.
pub fn get_plugin_setting(plugin_name: &str, key: &str) -> Option<serde_json::Value> {
    let config = read_ida_config();
    config
        .get("plugins")
        .and_then(|p| p.get(plugin_name))
        .and_then(|p| p.get("settings"))
        .and_then(|s| s.get(key))
        .cloned()
}

/// Set a plugin setting in `ida-config.json`.
pub fn set_plugin_setting(plugin_name: &str, key: &str, value: serde_json::Value) -> Result<()> {
    let mut config = read_ida_config();
    let plugins = config
        .as_object_mut()
        .unwrap()
        .entry("plugins")
        .or_insert(serde_json::json!({}));
    let plugin = plugins
        .as_object_mut()
        .unwrap()
        .entry(plugin_name)
        .or_insert(serde_json::json!({}));
    let settings = plugin
        .as_object_mut()
        .unwrap()
        .entry("settings")
        .or_insert(serde_json::json!({}));
    settings
        .as_object_mut()
        .unwrap()
        .insert(key.to_owned(), value);
    write_ida_config(&config)
}

/// Delete a plugin setting from `ida-config.json`.
pub fn del_plugin_setting(plugin_name: &str, key: &str) -> Result<()> {
    let mut config = read_ida_config();
    if let Some(settings) = config
        .get_mut("plugins")
        .and_then(|p| p.get_mut(plugin_name))
        .and_then(|p| p.get_mut("settings"))
        .and_then(|s| s.as_object_mut())
    {
        settings.remove(key);
        write_ida_config(&config)?;
    }
    Ok(())
}

/// Get all settings for a plugin from `ida-config.json`.
pub fn get_all_plugin_settings(plugin_name: &str) -> serde_json::Map<String, serde_json::Value> {
    let config = read_ida_config();
    config
        .get("plugins")
        .and_then(|p| p.get(plugin_name))
        .and_then(|p| p.get("settings"))
        .and_then(|s| s.as_object())
        .cloned()
        .unwrap_or_default()
}

/// Read plugin metadata from an installed plugin directory.
pub fn read_installed_metadata(name: &str) -> Result<crate::plugin::PluginMetadata> {
    let manifest_path = plugins_dir().join(name).join("ida-plugin.json");
    if !manifest_path.exists() {
        return Err(Error::PluginNotInstalled(name.into()));
    }
    let text = std::fs::read_to_string(&manifest_path)?;
    let manifest: crate::plugin::PluginManifest = serde_json::from_str(&text)?;
    Ok(manifest.metadata)
}

/// Get the plugin repository URL from `ida-config.json`.
pub fn get_repo_url() -> Option<String> {
    let config = read_ida_config();
    config
        .get("Settings")
        .and_then(|s| s.get("plugin-repository"))
        .and_then(|r| r.get("url"))
        .and_then(|u| u.as_str())
        .map(String::from)
}

/// Detect the IDA version from the current install directory.
pub fn detect_current_ida_version() -> Option<String> {
    let install_dir = crate::ida::current_install_dir()?;
    crate::ida::detect_ida_version(&install_dir)
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
