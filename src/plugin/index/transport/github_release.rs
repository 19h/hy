//! Select a direct-install release asset using the upstream decision order.

use crate::error::{Error, Result};
use crate::util::python_json::{self, Object, Text, Value};

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
    let value = python_json::decode(bytes)?;
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
        let empty_name = Text::from("");
        for asset in assets {
            let asset = object(asset, "asset")?;
            let name = match asset.get("name") {
                None => &empty_name,
                Some(Value::String(name)) => name,
                _ => return Err(invalid("asset name must be a string")),
            };
            if name.has_zip_suffix() {
                candidates.push((name.diagnostic(), asset));
            }
        }
    }
    let (name, asset) = match candidates.as_slice() {
        [(name, asset)] => (name, *asset),
        [] => {
            let tag = source.tag.as_deref().filter(|tag| !tag.is_empty()).unwrap_or("latest");
            return Err(Error::Other(format!(
                "No .zip asset found in release ({tag}) for {}/{}",
                source.owner, source.repository,
            )));
        }
        _ => {
            let names =
                candidates.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>().join(", ");
            return Err(Error::Other(format!(
                "Multiple .zip assets found in release: {names}. Cannot determine which to install."
            )));
        }
    };
    let size = asset.get("size");
    // Read the download field before comparing size, as the source does.
    let download =
        asset.get("browser_download_url").ok_or_else(|| invalid("missing browser_download_url"))?;
    if let Some(size) = oversized(size)? {
        return Err(Error::Other(format!(
            "Asset {name} ({size} bytes) exceeds maximum size limit ({MAX_DOWNLOAD_SIZE} bytes)"
        )));
    }
    match download {
        Value::String(url) => url.to_utf8(),
        _ => Err(invalid("browser_download_url must be a string")),
    }
}

fn object<'a>(value: &'a Value, description: &str) -> Result<&'a Object> {
    match value {
        Value::Object(object) => Ok(object),
        _ => Err(invalid(&format!("{description} must be an object"))),
    }
}

fn oversized(size: Option<&Value>) -> Result<Option<String>> {
    match size {
        None => Ok(None),
        Some(Value::Bool(value)) => Ok((u64::from(*value) > MAX_DOWNLOAD_SIZE).then(|| {
            if *value {
                "True"
            } else {
                "False"
            }
            .into()
        })),
        Some(Value::Integer(value)) => {
            Ok((value > &MAX_DOWNLOAD_SIZE.into()).then(|| value.to_string()))
        }
        Some(Value::Float(value)) => Ok((*value > MAX_DOWNLOAD_SIZE as f64)
            .then(|| crate::util::python_repr::float_repr(*value))),
        _ => Err(invalid("asset size must be a number")),
    }
}

fn invalid(message: &str) -> Error {
    Error::Other(format!("invalid GitHub release: {message}"))
}

#[cfg(test)]
mod tests;
