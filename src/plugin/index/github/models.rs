//! Validated catalogue models; serialized field names match upstream cache records.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};
use crate::plugin::metadata_values::deserialize_bool;
use crate::util::pydantic_integer::Integer;

mod graphql;

#[derive(Debug, Deserialize, Serialize)]
pub struct Repository {
    pub default_branch: Commit,
    pub releases: Vec<Release>,
    pub tags: Vec<Tag>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Release {
    pub name: String,
    pub tag_name: String,
    pub commit_hash: String,
    pub created_at: String,
    pub published_at: String,
    #[serde(deserialize_with = "deserialize_bool")]
    pub is_prerelease: bool,
    #[serde(deserialize_with = "deserialize_bool")]
    pub is_draft: bool,
    pub url: String,
    pub zipball_url: String,
    pub assets: Vec<Asset>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(try_from = "Value")]
pub struct Asset {
    pub name: String,
    pub download_url: String,
    pub size: Integer,
    pub content_type: String,
}

impl TryFrom<Value> for Asset {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self> {
        let field = |alias, name| {
            value
                .get(alias)
                .or_else(|| value.get(name))
                .cloned()
                .ok_or_else(|| Error::Other(format!("GitHub asset is missing field {alias}")))
        };
        // Pydantic prefers aliases even when their values fail validation.
        Ok(Self {
            name: serde_json::from_value(field("name", "name")?)?,
            download_url: serde_json::from_value(field("downloadUrl", "download_url")?)?,
            size: serde_json::from_value(field("size", "size")?)?,
            content_type: serde_json::from_value(field("contentType", "content_type")?)?,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Tag {
    pub tag_name: String,
    pub commit_hash: String,
    pub zipball_url: String,
    pub committed_date: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Commit {
    pub commit_hash: String,
    pub committed_date: String,
    pub zipball_url: String,
}

pub(super) fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64() != Some(0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
    }
}

#[cfg(test)]
mod tests;
