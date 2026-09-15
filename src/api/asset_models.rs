//! Asset response models and Pydantic-compatible numeric fields.

use serde::{Deserialize, Serialize};

mod integer;
mod metadata;

pub use integer::Integer;
pub use metadata::Metadata;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Asset {
    pub email: Option<String>,
    pub filename: String,
    #[serde(default)]
    pub size: Integer,
    pub key: String,
    pub code: Option<String>,
    pub created_at: Option<String>,
    pub expires_at: Option<String>,
    pub url: Option<String>,
    #[serde(default)]
    pub version: Integer,
    pub metadata: Option<Metadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagedAssets {
    pub offset: Integer,
    pub limit: Integer,
    pub total: Integer,
    pub items: Vec<Asset>,
}

#[cfg(test)]
mod tests;
