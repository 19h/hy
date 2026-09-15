//! Select a direct-install release asset using the upstream decision order.

use serde_json::{Map, Value};

use crate::error::{Error, Result};

use super::github_url::ReleaseSource;

const MAX_DOWNLOAD_SIZE: u64 = 100 * 1024 * 1024;

pub(super) fn endpoint(source: &ReleaseSource) -> String {
    let release = match source.tag.as_deref().filter(|tag| !tag.is_empty()) {
        Some(tag) => format!("tags/{tag}"),
        None => "latest".into(),
    };
    format!("/repos/{}/{}/releases/{release}", source.owner, source.repository)
}

pub(super) fn select(source: &ReleaseSource, bytes: &[u8]) -> Result<String> {
    let text = crate::util::json_encoding::decode(bytes)?;
    crate::util::json_numbers::validate_integer_limits(&text)?;
    let value: Value = serde_json::from_str(&text)?;
    let release = object(&value, "release")?;
    let mut candidates = Vec::new();
    if let Some(assets) = release.get("assets") {
        // Python also iterates empty strings and objects without error. Nonempty
        // ones yield string entries, which fail the asset.get(...) operation.
        let assets = match assets {
            Value::Array(assets) => assets.as_slice(),
            Value::Object(assets) if assets.is_empty() => &[],
            Value::String(assets) if assets.is_empty() => &[],
            _ => return Err(invalid("assets must yield asset objects")),
        };
        for asset in assets {
            let asset = object(asset, "asset")?;
            let name = match asset.get("name") {
                None => "",
                Some(Value::String(name)) => name,
                _ => return Err(invalid("asset name must be a string")),
            };
            if name.to_lowercase().ends_with(".zip") {
                candidates.push((name, asset));
            }
        }
    }
    let (name, asset) = match candidates.as_slice() {
        [(name, asset)] => (*name, *asset),
        [] => {
            let tag = source.tag.as_deref().filter(|tag| !tag.is_empty()).unwrap_or("latest");
            return Err(Error::Other(format!(
                "No .zip asset found in release ({tag}) for {}/{}",
                source.owner, source.repository,
            )));
        }
        _ => {
            let names = candidates.iter().map(|(name, _)| *name).collect::<Vec<_>>().join(", ");
            return Err(Error::Other(format!(
                "Multiple .zip assets found in release: {names}. Cannot determine which to install."
            )));
        }
    };
    let default_size = Value::Number(0.into());
    let size = asset.get("size").unwrap_or(&default_size);
    // Read the download field before comparing size, as the source does.
    let download =
        asset.get("browser_download_url").ok_or_else(|| invalid("missing browser_download_url"))?;
    if exceeds_limit(size)? {
        let size = crate::util::python_repr::json_str(size);
        return Err(Error::Other(format!(
            "Asset {name} ({size} bytes) exceeds maximum size limit ({MAX_DOWNLOAD_SIZE} bytes)"
        )));
    }
    download
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| invalid("browser_download_url must be a string"))
}

fn object<'a>(value: &'a Value, description: &str) -> Result<&'a Map<String, Value>> {
    value.as_object().ok_or_else(|| invalid(&format!("{description} must be an object")))
}

fn exceeds_limit(size: &Value) -> Result<bool> {
    match size {
        Value::Bool(_) => Ok(false),
        Value::Number(number) => {
            // The threshold is exactly representable as f64. Arbitrarily large
            // integers can overflow to infinity without changing this comparison.
            let number =
                number.to_string().parse::<f64>().map_err(|error| invalid(&error.to_string()))?;
            Ok(number > MAX_DOWNLOAD_SIZE as f64)
        }
        _ => Err(invalid("asset size must be a number")),
    }
}

fn invalid(message: &str) -> Error {
    Error::Other(format!("invalid GitHub release: {message}"))
}

#[cfg(test)]
mod tests;
