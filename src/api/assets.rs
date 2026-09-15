//! Shared asset upload lifecycle and canonical bucket/key paths.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;

use super::{ApiClient, Asset, AssetInteger};
use crate::error::{Error, Result};

pub fn asset_path(bucket: &str, key: &str) -> String {
    format!("/api/assets/{bucket}/{}", key.trim_start_matches('/'))
}

#[derive(Default, Serialize)]
pub struct UploadOptions {
    pub force: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_segments: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_emails: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_editions: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Map<String, Value>>,
}

#[derive(Serialize)]
struct UploadRequest {
    filename: String,
    size: u64,
    status: &'static str,
    checksum: String,
    #[serde(flatten)]
    options: UploadOptions,
}

/// Validated result, constructed after transfer and confirmation as upstream does.
#[derive(Deserialize)]
pub struct UploadedAsset {
    pub key: String,
    pub code: String,
    pub version: AssetInteger,
}

impl ApiClient {
    /// Upstream's APIError classes propagate through shortcode lookup: they are
    /// distinct from the HTTPStatusError caught by AssetAPI's optional interface.
    pub async fn shared_asset(&self, code: &str, version: &AssetInteger) -> Result<Option<Asset>> {
        self.get_json(&format!("/api/assets/s/{code}?version={version}")).await.map(Some)
    }

    pub async fn upload_asset(
        &self,
        bucket: &str,
        path: &Path,
        mut options: UploadOptions,
    ) -> Result<UploadedAsset> {
        let mut file = tokio::fs::File::open(path).await?;
        let metadata = file.metadata().await?;
        if !metadata.is_file() {
            return Err(Error::Other(format!("not a file: {}", path.display())));
        }
        let mut checksum = Sha256::new();
        let mut buffer = [0; 8192];
        loop {
            let count = file.read(&mut buffer).await?;
            if count == 0 {
                break;
            }
            checksum.update(&buffer[..count]);
        }
        options.code = options.code.filter(|code| !code.is_empty());
        let request = UploadRequest {
            filename: path
                .file_name()
                .ok_or_else(|| Error::Other("missing filename".into()))?
                .to_string_lossy()
                .into_owned(),
            size: metadata.len(),
            status: "active",
            checksum: format!("{:x}", checksum.finalize()),
            options,
        };
        let response: Map<String, Value> =
            self.post_json(&format!("/api/assets/{bucket}"), &request).await?;
        if let Some(url) = upload_url(response.get("url"))? {
            self.put_file(url, path).await?;
            let key = response.get("key").and_then(Value::as_str).ok_or_else(|| {
                Error::Other("upload ticket key must be a string for confirmation".into())
            })?;
            let _: Value = self.post_json(&asset_path(bucket, key), &serde_json::json!({})).await?;
        }
        Ok(serde_json::from_value(Value::Object(response))?)
    }
}

/// False-valued ticket URLs skip transfer, before the result model is validated.
fn upload_url(value: Option<&Value>) -> Result<Option<&str>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let present = match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64() != Some(0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
    };
    if !present {
        return Ok(None);
    }
    value.as_str().map(Some).ok_or_else(|| Error::Other("upload URL must be a string".into()))
}
