//! Discover older plugin formats for status reports, without treating them as managed.

use serde::Deserialize;

use crate::error::Result;

use super::{PluginManifest, plugins_dir};

pub enum UnmanagedKind {
    Incompatible,
    Legacy,
}

pub struct UnmanagedPlugin {
    pub name: String,
    pub version: Option<String>,
    pub path: String,
    pub kind: UnmanagedKind,
}

#[derive(Deserialize)]
struct MinimalManifest {
    #[serde(
        rename = "IDAMetadataDescriptorVersion",
        deserialize_with = "super::schema_version::deserialize"
    )]
    _descriptor_version: u32,
    plugin: MinimalPlugin,
}

#[derive(Deserialize)]
struct MinimalPlugin {
    name: String,
    version: Option<String>,
}

/// Upstream status lists minimal descriptors and top-level single-file plugins.
/// A valid modern descriptor with missing files or a mismatched name belongs to
/// neither group: it is an invalid managed installation, not an older format.
pub fn unmanaged_plugins() -> Result<Vec<UnmanagedPlugin>> {
    let root = plugins_dir();
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut plugins = Vec::new();
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let filename = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            let Ok(bytes) = std::fs::read(path.join("ida-plugin.json")) else {
                continue;
            };
            if serde_json::from_slice::<PluginManifest>(&bytes).is_ok() {
                continue;
            }
            let Ok(manifest) = serde_json::from_slice::<MinimalManifest>(&bytes) else {
                continue;
            };
            plugins.push(UnmanagedPlugin {
                name: manifest.plugin.name,
                version: manifest.plugin.version.filter(|version| !version.is_empty()),
                path: format!("{filename}/"),
                kind: UnmanagedKind::Incompatible,
            });
        } else if [".py", ".so", ".dll", ".dylib"]
            .iter()
            .any(|extension| filename.ends_with(extension))
        {
            plugins.push(UnmanagedPlugin {
                name: filename.clone(),
                version: None,
                path: filename,
                kind: UnmanagedKind::Legacy,
            });
        }
    }
    plugins.sort_by(|a, b| {
        matches!(a.kind, UnmanagedKind::Legacy)
            .cmp(&matches!(b.kind, UnmanagedKind::Legacy))
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(plugins)
}
