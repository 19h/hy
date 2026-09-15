//! Pydantic-compatible JSON coercions used by upstream metadata fields.

use serde::{Deserialize, Deserializer};
use serde_json::Value;

fn boolean(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(value) => Some(*value),
        Value::Number(value) => match value.as_f64()? {
            0.0 => Some(false),
            1.0 => Some(true),
            _ => None,
        },
        Value::String(value) => match value.to_ascii_lowercase().as_str() {
            "0" | "off" | "f" | "false" | "n" | "no" => Some(false),
            "1" | "on" | "t" | "true" | "y" | "yes" => Some(true),
            _ => None,
        },
        _ => None,
    }
}

pub(super) fn deserialize_bool<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<bool, D::Error> {
    boolean(&Value::deserialize(deserializer)?)
        .ok_or_else(|| serde::de::Error::custom("expected a boolean"))
}

pub(super) fn deserialize_default<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Value>, D::Error> {
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Null => Ok(None),
        Value::Bool(_) | Value::String(_) => Ok(Some(value)),
        Value::Number(_) => boolean(&value)
            .map(|value| Some(Value::Bool(value)))
            .ok_or_else(|| serde::de::Error::custom("setting default must be a string or boolean")),
        _ => Err(serde::de::Error::custom("setting default must be a string or boolean")),
    }
}
