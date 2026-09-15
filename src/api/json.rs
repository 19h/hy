//! API JSON uses CPython byte-encoding detection instead of HTTP text charsets.

use serde::de::DeserializeOwned;

use crate::error::{Error, Result};

pub(super) fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    let text = crate::util::json_encoding::decode(bytes)?;
    crate::util::json_numbers::validate_integer_limits(&text)?;
    Ok(serde_json::from_str(&text)?)
}

pub(super) fn status_error(status: u16, bytes: &[u8]) -> Error {
    let message = decode::<serde_json::Value>(bytes)
        .ok()
        .and_then(|value| value.get("message").map(crate::util::python_repr::json_str))
        .unwrap_or_else(|| format!("API request failed: {status}"));
    Error::from_status_message(status, Some(message))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod error_tests;
