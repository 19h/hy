//! Python JSON values, including arbitrary integers, nonfinite floats and surrogates.
//!
//! Parsing uses an explicit container stack. Consumers choose how to represent
//! Python strings at boundaries that require Unicode scalar values.

use indexmap::IndexMap;
use num_bigint::BigInt;

use crate::error::{Error, Result};

mod encoding;
mod parser;
mod string;

pub(crate) use string::Text;

#[derive(Debug)]
pub(crate) enum Value {
    Null,
    Bool(bool),
    Integer(BigInt),
    Float(f64),
    String(Text),
    Array(Vec<Value>),
    Object(Object),
}

impl Drop for Value {
    fn drop(&mut self) {
        let mut pending = Vec::new();
        self.detach_children(&mut pending);
        while let Some(mut value) = pending.pop() {
            value.detach_children(&mut pending);
        }
    }
}

impl Value {
    pub(crate) fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Self::Object(object) => object.get(key),
            _ => None,
        }
    }

    pub(crate) fn truthy(&self) -> bool {
        match self {
            Self::Null => false,
            Self::Bool(value) => *value,
            Self::Integer(value) => *value != BigInt::from(0),
            Self::Float(value) => *value != 0.0,
            Self::String(value) => !value.is_empty(),
            Self::Array(values) => !values.is_empty(),
            Self::Object(value) => !value.is_empty(),
        }
    }

    fn detach_children(&mut self, pending: &mut Vec<Value>) {
        match self {
            Self::Array(values) => pending.append(values),
            Self::Object(object) => pending.extend(std::mem::take(&mut object.0).into_values()),
            _ => {}
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Object(IndexMap<Text, Value>);

impl Object {
    pub(crate) fn keys(&self) -> impl Iterator<Item = &Text> {
        self.0.keys()
    }

    pub(crate) fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(&Text::from(key))
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

pub(crate) fn decode(bytes: &[u8]) -> Result<Value> {
    parser::parse(&encoding::decode(bytes)?)
}

/// Parse an already decoded Python string: unlike byte decoding, a BOM is invalid.
pub(crate) fn parse(text: &str) -> Result<Value> {
    parser::parse(&text.chars().map(u32::from).collect::<Vec<_>>())
}

fn invalid(message: impl std::fmt::Display) -> Error {
    Error::Other(format!("invalid JSON: {message}"))
}

#[cfg(test)]
mod tests;
