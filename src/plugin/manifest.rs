//! Versioned descriptors and plugin discovery inside directories and archives.

use serde::{Deserialize, Serialize};

use super::PluginMetadata;

mod selection;
pub(crate) use selection::select_archived_plugin;

/// Wrapper with schema version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    #[serde(rename = "$schema", skip_serializing)]
    pub _schema: Option<String>,
    #[serde(
        rename = "IDAMetadataDescriptorVersion",
        deserialize_with = "super::schema_version::deserialize"
    )]
    pub descriptor_version: u32,
    #[serde(rename = "plugin")]
    pub metadata: PluginMetadata,
}

/// One plugin descriptor and the lexical parent prefix used for archive extraction.
pub struct ArchivedPlugin {
    pub prefix: String,
    pub metadata: PluginMetadata,
}

/// Catalogue and installation readers skip invalid data but propagate member read errors.
pub(crate) fn read_archived_manifest<R: std::io::Read + std::io::Seek>(
    archive: &mut crate::util::python_zip::Archive<R>,
    index: usize,
) -> crate::error::Result<Option<(String, PluginManifest)>> {
    let name = archive.name_for_index(index).ok_or(zip::result::ZipError::FileNotFound)?;
    if !name.ends_with("ida-plugin.json") {
        return Ok(None);
    }
    let path = name.to_owned();
    let bytes = archive.read(&path)?;
    match serde_json::from_slice(&bytes) {
        Ok(manifest) => Ok(Some((path, manifest))),
        Err(error) => {
            tracing::debug!(%path, %error, "ignoring invalid plugin descriptor");
            Ok(None)
        }
    }
}

/// Read plugin metadata from a source directory containing `ida-plugin.json`.
pub fn read_metadata_from_directory(dir: &std::path::Path) -> crate::error::Result<PluginMetadata> {
    let manifest_path = dir.join("ida-plugin.json");
    if !manifest_path.is_file() {
        return Err(crate::error::Error::PluginInstall(format!(
            "ida-plugin.json not found in {}",
            dir.display()
        )));
    }
    let text = std::fs::read_to_string(&manifest_path)?;
    let manifest: PluginManifest = serde_json::from_str(&text)?;
    super::validate_directory_files(&manifest.metadata, dir)?;
    Ok(manifest.metadata)
}
