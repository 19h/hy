//! Setting descriptors, value validation and ordered JSON representation.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SettingType {
    String,
    Boolean,
}

impl std::fmt::Display for SettingType {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::String => "string",
            Self::Boolean => "boolean",
        })
    }
}

/// A single plugin setting descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSetting {
    #[serde(rename = "type")]
    pub setting_type: SettingType,
    pub name: String,
    #[serde(rename = "documentation")]
    pub description: Option<String>,
    #[serde(default, deserialize_with = "super::metadata_values::deserialize_default")]
    pub default: Option<serde_json::Value>,
    #[serde(deserialize_with = "super::metadata_values::deserialize_bool")]
    pub required: bool,
    pub choices: Option<Vec<String>>,
    #[serde(rename = "validation_pattern")]
    pub pattern: Option<String>,
    #[serde(
        default = "default_prompt",
        deserialize_with = "super::metadata_values::deserialize_bool"
    )]
    pub prompt: bool,
    #[serde(default, deserialize_with = "super::metadata_values::deserialize_bool")]
    pub secret: bool,
}

fn default_prompt() -> bool {
    true
}

pub(super) fn serialize_settings<S: serde::Serializer>(
    settings: &IndexMap<String, PluginSetting>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    #[derive(Serialize)]
    struct NamedSetting<'a> {
        key: &'a str,
        #[serde(flatten)]
        descriptor: &'a PluginSetting,
    }
    let entries: Vec<_> = settings
        .iter()
        .map(|(key, descriptor)| NamedSetting {
            key,
            descriptor,
        })
        .collect();
    entries.serialize(serializer)
}

impl PluginSetting {
    pub fn parse_value(&self, key: &str, raw: &str) -> crate::error::Result<serde_json::Value> {
        let value = match self.setting_type {
            SettingType::Boolean => match raw.to_ascii_lowercase().as_str() {
                "true" => serde_json::Value::Bool(true),
                "false" => serde_json::Value::Bool(false),
                _ => {
                    return Err(crate::error::Error::Other(format!("{key} must be true or false")));
                }
            },
            SettingType::String => serde_json::Value::String(raw.into()),
        };
        Ok(value)
    }

    pub fn is_promptable(&self) -> bool {
        self.prompt
    }

    pub fn validate_value(&self, key: &str, value: &serde_json::Value) -> crate::error::Result<()> {
        use crate::error::Error;

        match self.setting_type {
            SettingType::Boolean if value.is_boolean() => return Ok(()),
            SettingType::String if value.is_string() => (),
            _ => return Err(Error::Other(format!("{key} must be a {}", self.setting_type))),
        }
        let text = value.as_str().expect("string type checked above");
        if let Some(choices) = &self.choices
            && !choices.iter().any(|choice| choice == text)
        {
            return Err(Error::Other(format!("invalid choice for {key}")));
        }
        if let Some(pattern) = &self.pattern
            && !crate::util::python_regex::matches_prefix(pattern, text)
                .map_err(|error| Error::Other(format!("validation pattern for {key}: {error}")))?
        {
            return Err(Error::Other(format!("{key} does not match its validation pattern")));
        }
        Ok(())
    }
}

pub(super) fn setting_descriptors<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<IndexMap<String, PluginSetting>, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    if let Some(items) = v.as_array() {
        let mut settings = IndexMap::new();
        for item in items {
            let key = item
                .get("key")
                .and_then(|v| v.as_str())
                .ok_or_else(|| serde::de::Error::custom("setting missing key"))?;
            let descriptor =
                serde_json::from_value(item.clone()).map_err(serde::de::Error::custom)?;
            if settings.insert(key.to_owned(), descriptor).is_some() {
                return Err(serde::de::Error::custom("duplicate setting key"));
            }
        }
        Ok(settings)
    } else {
        Err(serde::de::Error::custom("settings must be an array of descriptors"))
    }
}
