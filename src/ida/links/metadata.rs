//! Validate Python JSON metadata while preserving the original downloaded bytes.

use crate::error::{Error, Result};

pub(super) fn validate(bytes: &[u8]) -> Result<()> {
    let decoded = crate::util::json_encoding::validation_text(bytes).map_err(invalid)?;
    let syntax = crate::util::json_numbers::normalize_for_validation(&decoded).map_err(invalid)?;
    // IgnoredAny validates syntax iteratively without constructing a value tree.
    // It skips Unicode checks, so decoding must have validated the bytes first.
    serde_json::from_slice::<serde::de::IgnoredAny>(&syntax)
        .map_err(|error| invalid(error.to_string()))?;
    Ok(())
}

fn invalid(reason: impl std::fmt::Display) -> Error {
    Error::Other(format!("KE metadata is not valid JSON: {reason}"))
}

#[cfg(test)]
mod tests;
