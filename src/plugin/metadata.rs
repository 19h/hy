//! Plugin metadata types (ida-plugin.json schema).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Full plugin metadata as described by `ida-plugin.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    pub entry_point: Option<String>,
    pub author: Option<String>,
    pub authors: Option<Vec<Contact>>,
    pub license: Option<String>,
    pub urls: Option<Urls>,
    pub categories: Option<Vec<String>>,
    pub keywords: Option<Vec<String>>,
    pub ida_versions: Option<Vec<String>>,
    pub platforms: Option<Vec<String>>,
    pub logo_path: Option<String>,
    pub python_dependencies: Option<Vec<String>>,
    pub settings: Option<HashMap<String, PluginSetting>>,
}

/// Author / contact information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub name: String,
    pub email: Option<String>,
    pub url: Option<String>,
}

/// Plugin URLs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Urls {
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub documentation: Option<String>,
    pub issues: Option<String>,
}

/// A single plugin setting descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSetting {
    #[serde(rename = "type")]
    pub setting_type: String, // "string" | "boolean"
    pub description: Option<String>,
    pub default: Option<serde_json::Value>,
    pub required: Option<bool>,
    pub choices: Option<Vec<String>>,
    pub pattern: Option<String>,
    pub prompt: Option<String>,
}

/// Wrapper with schema version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(flatten)]
    pub metadata: PluginMetadata,
}

/// Minimal metadata for legacy / single-file plugins.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinimalPluginMetadata {
    pub name: String,
    pub version: Option<String>,
    pub path: String,
}

// ── compatibility helpers ───────────────────────────────────────────────

/// Check whether a plugin is compatible with a given IDA version.
pub fn is_ida_version_compatible(plugin: &PluginMetadata, ida_version: &str) -> bool {
    match &plugin.ida_versions {
        None => true, // no restriction → compatible
        Some(versions) => {
            if versions.is_empty() {
                return true;
            }
            // Simple substring / prefix matching.
            versions.iter().any(|spec| {
                ida_version.starts_with(spec)
                    || spec.starts_with(ida_version)
                    || spec == "*"
                    || spec == ">=3.0"
            })
        }
    }
}

/// All known IDA platform identifiers.
pub const PLATFORM_WINDOWS: &str = "win";
pub const PLATFORM_LINUX: &str = "linux";
pub const PLATFORM_MACOS_INTEL: &str = "macx64";
pub const PLATFORM_MACOS_ARM: &str = "macarm";

/// Check whether a plugin is compatible with the current platform.
pub fn is_platform_compatible(plugin: &PluginMetadata) -> bool {
    match &plugin.platforms {
        None => true,
        Some(platforms) => {
            if platforms.is_empty() {
                return true;
            }
            let current = current_platform();
            platforms.iter().any(|p| p == current || p == "all")
        }
    }
}

fn current_platform() -> &'static str {
    if cfg!(target_os = "windows") {
        PLATFORM_WINDOWS
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            PLATFORM_MACOS_ARM
        } else {
            PLATFORM_MACOS_INTEL
        }
    } else {
        PLATFORM_LINUX
    }
}

/// Read plugin metadata from a zip archive.
pub fn read_metadata_from_archive(
    archive_path: &std::path::Path,
) -> crate::error::Result<PluginMetadata> {
    let file = std::fs::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    // Look for ida-plugin.json in root or one level deep.
    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;
        let name = entry.name().to_owned();
        if name == "ida-plugin.json" || name.ends_with("/ida-plugin.json") {
            let depth = name.matches('/').count();
            if depth <= 1 {
                let manifest: PluginManifest = serde_json::from_reader(entry)?;
                return Ok(manifest.metadata);
            }
        }
    }

    Err(crate::error::Error::PluginInstall(
        "ida-plugin.json not found in archive".into(),
    ))
}
