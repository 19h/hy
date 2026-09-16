//! Field validation shared by cached records and GraphQL conversion.

use crate::error::{Error, Result};
use crate::util::{
    pydantic_integer::Integer,
    python_json::{Text, Value},
};

pub(super) fn required<'a>(value: &'a Value, name: &str) -> Result<&'a Value> {
    value.get(name).ok_or_else(|| Error::Other(format!("GitHub record is missing field {name}")))
}

pub(super) fn text(value: &Value) -> Result<Text> {
    match value {
        Value::String(text) => Ok(text.clone()),
        _ => Err(Error::Other("expected a string in GitHub metadata".into())),
    }
}

pub(super) fn string(value: &Value, name: &str) -> Result<Text> {
    text(required(value, name)?)
}

pub(super) fn integer(value: &Value) -> Result<Integer> {
    Integer::from_python(value)
        .ok_or_else(|| Error::Other("expected an integer in GitHub metadata".into()))
}

pub(super) fn boolean(value: &Value) -> Result<bool> {
    let parsed = match value {
        Value::Bool(value) => Some(*value),
        Value::Integer(value) if *value == 0.into() => Some(false),
        Value::Integer(value) if *value == 1.into() => Some(true),
        Value::Float(value) if *value == 0.0 => Some(false),
        Value::Float(value) if *value == 1.0 => Some(true),
        Value::String(value) => {
            value.to_utf8().ok().and_then(|value| match value.to_ascii_lowercase().as_str() {
                "0" | "off" | "f" | "false" | "n" | "no" => Some(false),
                "1" | "on" | "t" | "true" | "y" | "yes" => Some(true),
                _ => None,
            })
        }
        _ => None,
    };
    parsed.ok_or_else(|| Error::Other("expected a boolean in GitHub metadata".into()))
}

pub(super) fn list<T>(value: &Value, parse: impl Fn(&Value) -> Result<T>) -> Result<Vec<T>> {
    match value {
        Value::Array(values) => values.iter().map(parse).collect(),
        _ => Err(invalid_collection()),
    }
}

pub(super) fn invalid_collection() -> Error {
    Error::Other("invalid GitHub metadata collection".into())
}
