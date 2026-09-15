//! Match the source's Pydantic-to-JSON, sorted, four-space snapshot output.

use super::Snapshot;
use crate::error::Result;
use crate::util::{json_format, json_numbers};

impl Snapshot {
    pub fn to_json(&self) -> Result<String> {
        let output = json_format::sorted_ascii(&serde_json::to_value(self)?, "    ");
        json_numbers::validate_integer_limits(&output)?;
        Ok(output)
    }
}

#[cfg(test)]
mod tests;
