//! JSON metadata equality follows Python's numeric equality inside containers.

use std::collections::HashMap;

use num_bigint::BigInt;
use num_traits::FromPrimitive;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Metadata(HashMap<String, Value>);

impl std::ops::Deref for Metadata {
    type Target = HashMap<String, Value>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PartialEq for Metadata {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .all(|(key, value)| other.get(key).is_some_and(|other| equal(value, other)))
    }
}

fn equal(left: &Value, right: &Value) -> bool {
    if let (Some(left), Some(right)) = (number(left), number(right)) {
        return match (left, right) {
            (Number::Integer(left), Number::Integer(right)) => left == right,
            (Number::Float(left), Number::Float(right)) => left == right,
            (Number::Integer(integer), Number::Float(float))
            | (Number::Float(float), Number::Integer(integer)) => {
                float.is_finite()
                    && float.fract() == 0.0
                    && BigInt::from_f64(float).is_some_and(|value| value == integer)
            }
        };
    }
    match (left, right) {
        (Value::Array(left), Value::Array(right)) => {
            left.len() == right.len()
                && left.iter().zip(right).all(|(left, right)| equal(left, right))
        }
        (Value::Object(left), Value::Object(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .all(|(key, value)| right.get(key).is_some_and(|right| equal(value, right)))
        }
        _ => left == right,
    }
}

enum Number {
    Integer(BigInt),
    Float(f64),
}

fn number(value: &Value) -> Option<Number> {
    match value {
        Value::Bool(value) => Some(Number::Integer(BigInt::from(u8::from(*value)))),
        Value::Number(value) => {
            let text = value.to_string();
            if text.contains(['.', 'e', 'E']) {
                text.parse().ok().map(Number::Float)
            } else {
                BigInt::parse_bytes(text.as_bytes(), 10).map(Number::Integer)
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_equality_preserves_integer_precision_and_container_shape() {
        for (left, right, expected) in [
            ("true", "1.0", true),
            ("false", "-0.0", true),
            ("9007199254740992", "9007199254740992.0", true),
            ("9007199254740993", "9007199254740992.0", false),
            ("18446744073709551616", "18446744073709551616.0", true),
            ("18446744073709551617", "18446744073709551616.0", false),
            ("[true, 2]", "[1, 2.0]", true),
            ("[true, 2]", "[2, true]", false),
            ("[1]", "1", false),
            (r#"{"a":true,"b":2}"#, r#"{"b":2.0,"a":1}"#, true),
            (r#"{"a":1}"#, r#"{"b":1}"#, false),
        ] {
            let left: Value = serde_json::from_str(left).unwrap();
            let right: Value = serde_json::from_str(right).unwrap();
            assert_eq!(equal(&left, &right), expected, "{left} == {right}");
            assert_eq!(equal(&right, &left), expected, "{right} == {left}");
        }
    }
}
