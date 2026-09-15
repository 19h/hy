//! Python equality for JSON model fields, including bool/int/float equivalence.

use num_bigint::BigInt;
use num_traits::FromPrimitive;
use serde_json::Value;

pub(super) fn equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Null, Value::Null) => true,
        (Value::String(left), Value::String(right)) => left == right,
        (Value::Array(left), Value::Array(right)) => {
            left.len() == right.len() && left.iter().zip(right).all(|(a, b)| equal(a, b))
        }
        (Value::Object(left), Value::Object(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .all(|(key, value)| right.get(key).is_some_and(|other| equal(value, other)))
        }
        _ => match (numeric(left), numeric(right)) {
            (Some(Numeric::Integer(left)), Some(Numeric::Integer(right))) => left == right,
            (Some(Numeric::Float(left)), Some(Numeric::Float(right))) => left == right,
            (Some(Numeric::Integer(integer)), Some(Numeric::Float(float)))
            | (Some(Numeric::Float(float)), Some(Numeric::Integer(integer))) => {
                float.fract() == 0.0
                    && BigInt::from_f64(float).is_some_and(|value| value == integer)
            }
            _ => false,
        },
    }
}

enum Numeric {
    Integer(BigInt),
    Float(f64),
}

fn numeric(value: &Value) -> Option<Numeric> {
    match value {
        Value::Bool(value) => Some(Numeric::Integer(BigInt::from(u8::from(*value)))),
        Value::Number(number) => {
            let text = number.as_str();
            if text.contains(['.', 'e', 'E']) {
                text.parse().ok().map(Numeric::Float)
            } else {
                BigInt::parse_bytes(text.as_bytes(), 10).map(Numeric::Integer)
            }
        }
        _ => None,
    }
}
