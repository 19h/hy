//! Bundle manifest models, target serialization and manifest validation.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::PipTarget;
use crate::error::{Error, Result};
use crate::util::python_zip::Archive;

mod datetime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleCreatedBy {
    pub tool: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleTargetPlatformTag {
    pub id: String,
    pub ida_platform: String,
    pub python_version: String,
    pub implementation: String,
    pub abis: Vec<String>,
    pub pip_platform_tags: Vec<String>,
    pub wheelhouse: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleManifest {
    #[serde(deserialize_with = "crate::plugin::schema_version::deserialize")]
    pub version: u32,
    pub kind: String,
    #[serde(deserialize_with = "datetime::deserialize")]
    pub built_at: String,
    pub created_by: BundleCreatedBy,
    pub target_platform_tags: Vec<BundleTargetPlatformTag>,
}

impl BundleTargetPlatformTag {
    pub fn from_target(target: &PipTarget) -> Result<Self> {
        Ok(Self {
            id: target.id(),
            ida_platform: target.ida_platform.clone(),
            python_version: target.python_version.clone(),
            implementation: "cp".into(),
            abis: target.abis(),
            pip_platform_tags: target.pip_platform_tags()?,
            wheelhouse: format!("dependencies/python/{}", target.id()),
        })
    }
}

pub(super) fn read_from_archive(
    archive: &mut Archive<std::fs::File>,
    path: &Path,
) -> Result<BundleManifest> {
    let bytes = archive
        .read("plugin-bundle.json")
        .map_err(|_| Error::Other(format!("{} is not a plugin bundle", path.display())))?;
    let text = String::from_utf8(bytes)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    let manifest: BundleManifest = serde_json::from_str(&text)
        .map_err(|e| Error::Other(format!("invalid plugin-bundle.json: {e}")))?;
    if manifest.kind != "hcli-plugin-bundle" {
        return Err(Error::Other(format!(
            "unsupported bundle (kind: {}, version: {})",
            manifest.kind, manifest.version
        )));
    }
    let mut targets = std::collections::HashSet::new();
    for target in &manifest.target_platform_tags {
        if !targets.insert(&target.id) {
            return Err(Error::Other(format!("duplicate bundle target ID: {}", target.id)));
        }
        validate_bundle_path(&target.wheelhouse)?;
    }
    Ok(manifest)
}

pub fn validate_bundle_path(path: &str) -> Result<()> {
    if path.starts_with('/') || path.split('/').any(|part| part == "..") || path.contains('\\') {
        return Err(Error::PluginInstall(format!("unsafe bundle path: {path}")));
    }
    Ok(())
}
