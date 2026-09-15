//! Literal version 1 shared by descriptor, repository and bundle schemas.

use serde::{Deserialize, Deserializer};

pub(super) fn one() -> u32 {
    1
}

pub(super) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    if value == serde_json::Value::Bool(true) || value.as_f64() == Some(1.0) {
        Ok(1)
    } else {
        Err(serde::de::Error::custom("Input should be 1"))
    }
}
