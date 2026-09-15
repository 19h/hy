//! Repository wire models use the same complete descriptor as local archives.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::plugin::{PluginManifest, schema_version};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    #[serde(default = "schema_version::one", deserialize_with = "schema_version::deserialize")]
    pub version: u32,
    pub plugins: Vec<Plugin>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plugin {
    pub name: String,
    pub host: String,
    pub versions: IndexMap<String, Vec<Location>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub url: String,
    pub sha256: String,
    #[serde(rename = "metadata")]
    pub descriptor: PluginManifest,
}

#[cfg(test)]
mod tests;
