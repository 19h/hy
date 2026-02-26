//! API data models (mirrors the Python Pydantic models).
//!
//! All models are kept even if not currently referenced, since they are part
//! of the API schema and will be used as features are exercised.

use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

/// Deserialize a value that might be `null` in JSON into `T::default()`.
/// Unlike `#[serde(default)]` alone, this handles explicit `null` values.
fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// Deserialize an i64 that might arrive as a JSON string (e.g. `"1000"` instead of `1000`).
fn string_or_i64<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrNum {
        Num(i64),
        Str(String),
    }
    match StringOrNum::deserialize(deserializer)? {
        StringOrNum::Num(n) => Ok(n),
        StringOrNum::Str(s) => s.parse().map_err(serde::de::Error::custom),
    }
}

// ── Auth ────────────────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUser {
    pub email: String,
}

// ── Customer ────────────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Customer {
    pub id: Option<i64>,
    pub email: Option<String>,
    pub notes: Option<String>,
    pub company: Option<String>,
    pub country: Option<String>,
    pub last_name: Option<String>,
    pub created_at: Option<String>,
    pub first_name: Option<String>,
    pub updated_at: Option<String>,
    pub vat_number: Option<String>,
    pub acquired_at: Option<String>,
    pub cf_reseller: Option<bool>,
    pub cf_coupon_id: Option<String>,
    pub cf_kyc_status: Option<String>,
    pub net_term_days: Option<i32>,
    pub cf_customer_key: Option<String>,
    pub cf_gdpr_deleted: Option<bool>,
    pub cf_customer_category: Option<String>,
    pub chargebee_customer_id: Option<String>,
    pub cf_notifications_enabled: Option<bool>,
}

#[allow(dead_code)]
impl Customer {
    /// Human-readable display name.
    pub fn display_name(&self) -> String {
        let mut parts = Vec::new();
        if let Some(ref f) = self.first_name {
            parts.push(f.as_str());
        }
        if let Some(ref l) = self.last_name {
            parts.push(l.as_str());
        }
        if let Some(ref c) = self.company {
            parts.push(c.as_str());
        }
        if parts.is_empty() {
            self.email.clone().unwrap_or_else(|| "Unknown".into())
        } else {
            parts.join(" ")
        }
    }
}

// ── License ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub family: Option<String>,
    pub catalog: String,
    pub edition: Option<String>,
    pub platform: Option<String>,
    pub ui_label: Option<String>,
    pub base_code: Option<String>,
    pub update_code: Option<String>,
    pub product_type: String,
    pub product_subtype: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Addon {
    pub id: Option<i64>,
    pub pubhash: Option<String>,
    pub license_key: Option<String>,
    pub seats: Option<i32>,
    pub password: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub product_code: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub activation_owner: Option<String>,
    pub activation_owner_type: Option<String>,
    pub product: Option<Product>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edition {
    pub id: Option<i64>,
    pub tags: Option<Vec<String>>,
    pub plan_id: Option<String>,
    pub max_items: Option<i32>,
    pub plan_name: Option<String>,
    pub edition_id: Option<String>,
    pub edition_name: Option<String>,
    pub build_edition_id: Option<String>,
    pub build_product_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct License {
    pub id: Option<i64>,
    pub pubhash: Option<String>,
    pub plan_id: Option<String>,
    pub license_key: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub license_type: Option<String>,
    pub seats: Option<i32>,
    pub password: Option<String>,
    pub product_code: Option<String>,
    pub customer_id: Option<i64>,
    pub end_customer_id: Option<i64>,
    pub activation_owner: Option<String>,
    pub activation_owner_type: Option<String>,
    pub status: Option<String>,
    pub comment: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub generation_date: Option<String>,
    pub download_date: Option<String>,
    pub addons: Option<Vec<Addon>>,
    pub edition: Option<Edition>,
    pub asset_types: Option<Vec<String>>,
    pub format_version: Option<String>,
    pub product_catalog: Option<String>,
    pub end_customer_visible: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagedLicenses {
    pub items: Vec<License>,
    pub total: i64,
}

// ── API Keys ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub name: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub request_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyToken {
    pub key: String,
}

// ── Assets / File Sharing ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub email: Option<String>,
    #[serde(default)]
    pub filename: String,
    #[serde(default, deserialize_with = "null_as_default")]
    pub size: u64,
    pub key: String,
    pub code: Option<String>,
    pub created_at: Option<String>,
    pub expires_at: Option<String>,
    pub url: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    pub version: u32,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagedAssets {
    #[serde(deserialize_with = "string_or_i64")]
    pub offset: i64,
    #[serde(deserialize_with = "string_or_i64")]
    pub limit: i64,
    #[serde(deserialize_with = "string_or_i64")]
    pub total: i64,
    pub items: Vec<Asset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNode {
    pub name: String,
    #[serde(rename = "type", default = "default_tree_type")]
    pub node_type: String,
    pub children: Option<Vec<TreeNode>>,
    pub asset: Option<Asset>,
}

fn default_tree_type() -> String {
    "file".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub tag: String,
    pub description: String,
    pub bucket: String,
    pub key: String,
    pub category: String,
    pub channel: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagsResponse {
    pub tags: Vec<Tag>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadResponse {
    pub bucket: String,
    pub key: String,
    pub version: u32,
    pub code: String,
    pub url: String,
    pub download_url: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadRequest {
    pub filename: String,
    pub size: u64,
    pub force: bool,
    pub status: String,
    pub checksum: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_segments: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_emails: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_editions: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

// ── Bucket ──────────────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketMetadata {
    pub name: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequiredField {
    pub description: String,
    pub example: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bucket {
    pub filename: String,
    pub metadata: BucketMetadata,
    #[serde(rename = "requiredMetadata")]
    pub required_metadata: HashMap<String, RequiredField>,
}
