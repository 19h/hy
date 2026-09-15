//! Plugin metadata types (ida-plugin.json schema).

use super::settings::PluginSetting;
use super::{compatibility, parse_version, settings};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Full plugin metadata as described by `ida-plugin.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(remote = "Self", rename_all = "camelCase")]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub entry_point: String,
    #[serde(default)]
    pub authors: Vec<Contact>,
    #[serde(default)]
    pub maintainers: Vec<Contact>,
    pub license: Option<String>,
    pub urls: Urls,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(
        default = "compatibility::all_ida_versions",
        deserialize_with = "compatibility::deserialize_ida_versions"
    )]
    pub ida_versions: Vec<String>,
    #[serde(
        default = "compatibility::all_platforms",
        deserialize_with = "compatibility::deserialize_platforms"
    )]
    pub platforms: Vec<String>,
    pub logo_path: Option<String>,
    #[serde(default)]
    pub python_dependencies: super::PythonDependencies,
    #[serde(
        default,
        deserialize_with = "settings::setting_descriptors",
        serialize_with = "settings::serialize_settings"
    )]
    pub settings: IndexMap<String, PluginSetting>,
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

// Remote derive supplies field decoding without duplicating the model's fields.
// The trait implementation then enforces invariants for every metadata reader.
impl<'de> Deserialize<'de> for PluginMetadata {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let metadata = Self::deserialize(deserializer)?;
        super::validation::validate(&metadata).map_err(serde::de::Error::custom)?;
        Ok(metadata)
    }
}

impl Serialize for PluginMetadata {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Self::serialize(self, serializer)
    }
}

/// Author / contact information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub name: Option<String>,
    pub email: String,
}

/// Plugin URLs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Urls {
    pub homepage: Option<String>,
    pub repository: String,
}

impl PluginMetadata {
    pub fn setting(&self, key: &str) -> crate::error::Result<&PluginSetting> {
        self.settings.get(key).ok_or_else(|| {
            crate::error::Error::NotFound(format!("plugin setting: {}.{key}", self.name))
        })
    }

    /// The repository identity is independent of the archive's download URL.
    pub fn normalized_host(&self) -> crate::error::Result<String> {
        crate::plugin::index::normalize_host(&self.urls.repository)
    }

    /// Detect identity changes between inspection and staging of the same source.
    pub fn verify_prepared_identity(&self, expected: &Self) -> crate::error::Result<()> {
        use crate::error::Error;

        if !self.name.eq_ignore_ascii_case(&expected.name)
            || self.normalized_host()? != expected.normalized_host()?
            || parse_version(&self.version).is_none()
            || parse_version(&self.version) != parse_version(&expected.version)
        {
            return Err(Error::PluginInstall(format!(
                "plugin identity changed while preparing {}=={}",
                expected.name, expected.version
            )));
        }
        Ok(())
    }
}

/// Upstream's checked-in schema, pinned in docs/parity.md.
pub fn ida_plugin_json_schema() -> serde_json::Value {
    serde_json::from_str(include_str!("../../schemas/ida-plugin.json"))
        .expect("bundled plugin schema must be valid JSON")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::PluginManifest;
    use serde_json::json;

    #[test]
    fn manifest_round_trip_preserves_upstream_fields() {
        let input = json!({
            "IDAMetadataDescriptorVersion": 1,
            "plugin": {
                "name": "example",
                "version": "1.0.0",
                "entryPoint": "plugin.py",
                "urls": {"repository": "https://github.com/example/plugin"},
                "description": null,
                "maintainers": [{"email": "maintainer@example.test"}],
                "customMetadata": {"value": 42},
                "settings": [{
                    "key": "token",
                    "name": "Access token",
                    "type": "string",
                    "required": true,
                    "secret": true,
                    "validation_pattern": "token-[a-z]+",
                    "documentation": "A test token",
                }],
            },
        });
        let manifest: PluginManifest = serde_json::from_value(input).unwrap();
        let serialized = serde_json::to_value(&manifest).unwrap();
        assert_eq!(serialized["IDAMetadataDescriptorVersion"], 1);
        assert_eq!(serialized["plugin"]["customMetadata"]["value"], 42);
        assert_eq!(serialized["plugin"]["maintainers"][0]["email"], "maintainer@example.test");
        let setting = &serialized["plugin"]["settings"][0];
        assert_eq!(setting["key"], "token");
        assert_eq!(setting["secret"], true);
        assert_eq!(setting["validation_pattern"], "token-[a-z]+");
        assert_eq!(setting["documentation"], "A test token");
        serde_json::from_value::<PluginManifest>(serialized).unwrap();
    }

    #[test]
    fn validates_settings_with_upstream_match_anchoring() {
        let descriptor: PluginSetting = serde_json::from_value(json!({
            "name": "Token", "required": false, "type": "string", "validation_pattern": "token-[a-z]+",
        }))
        .unwrap();
        assert!(descriptor.validate_value("token", &json!("token-abc")).is_ok());
        assert!(descriptor.validate_value("token", &json!("prefix-token-abc")).is_err());
        assert!(descriptor.validate_value("token", &json!(false)).is_err());
    }

    #[test]
    fn schema_describes_nested_manifests_and_setting_arrays() {
        let schema = ida_plugin_json_schema();
        assert_eq!(schema["properties"]["IDAMetadataDescriptorVersion"]["const"], 1);
        assert_eq!(schema["$defs"]["PluginMetadata"]["properties"]["settings"]["type"], "array");
        assert!(
            schema["$defs"]["PluginMetadata"]["properties"]["platforms"]["items"]["enum"]
                .as_array()
                .unwrap()
                .contains(&json!("windows-aarch64"))
        );
    }
}
