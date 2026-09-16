//! Pydantic 2.12 integer fields, preserving arbitrary-precision JSON integers.

use num_bigint::BigInt;
use num_traits::ToPrimitive;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

const MAX_DECIMAL_DIGITS: usize = 4300;

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Integer(BigInt);

impl From<i64> for Integer {
    fn from(value: i64) -> Self {
        Self(value.into())
    }
}

impl Integer {
    pub(crate) fn to_python(&self) -> crate::util::python_json::Value {
        crate::util::python_json::Value::Integer(self.0.clone())
    }

    pub(crate) fn from_python(value: &crate::util::python_json::Value) -> Option<Self> {
        use crate::util::python_json::Value;
        match value {
            Value::Integer(value) => Some(Self(value.clone())),
            Value::Bool(value) => Some(Self(BigInt::from(u8::from(*value)))),
            Value::Float(value) => from_float(*value).map(Self),
            Value::String(value) => from_string(&value.to_utf8().ok()?).map(Self),
            _ => None,
        }
    }

    pub(crate) fn to_u64(&self) -> Option<u64> {
        self.0.to_u64()
    }
}

impl std::fmt::Display for Integer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Serialize for Integer {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0
            .to_string()
            .parse::<serde_json::Number>()
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Integer {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        let integer = match value {
            Value::Bool(value) => Some(BigInt::from(u8::from(value))),
            Value::String(value) => from_string(&value),
            Value::Number(value) => {
                let text = value.to_string();
                if text.contains(['.', 'e', 'E']) {
                    value.as_f64().and_then(from_float)
                } else {
                    // HTTPX first decodes JSON with CPython. Its integer limit
                    // excludes the sign, unlike Pydantic's numeric-string path.
                    let digits = text.strip_prefix('-').unwrap_or(&text);
                    (digits.len() <= MAX_DECIMAL_DIGITS)
                        .then(|| BigInt::parse_bytes(text.as_bytes(), 10))
                        .flatten()
                }
            }
            _ => None,
        };
        integer.map(Self).ok_or_else(|| serde::de::Error::custom("expected an integer"))
    }
}

fn from_float(value: f64) -> Option<BigInt> {
    (value.is_finite()
        && value.fract() == 0.0
        && value > i64::MIN as f64
        && value < i64::MAX as f64)
        .then(|| BigInt::from(value as i64))
}

fn from_string(value: &str) -> Option<BigInt> {
    match direct(value) {
        Direct::Integer(value) => return Some(value),
        Direct::TooLong => return None,
        Direct::Clean => (),
    }
    let mut text = value.trim();
    if let Some(rest) = text.strip_prefix('+') {
        if rest.starts_with('-') {
            return None;
        }
        text = rest;
    }
    let negative = text.starts_with('-');
    if negative {
        text = &text[1..];
        if text.starts_with(['-', '+']) {
            return None;
        }
    }
    text = leading_zeros(text)?;
    if let Some((whole, fraction)) = text.split_once('.')
        && !fraction.is_empty()
        && fraction.bytes().all(|byte| byte == b'0')
    {
        text = whole;
    }
    if text.starts_with('_') || text.ends_with('_') || text.contains("__") {
        return None;
    }
    let mut cleaned = if negative {
        "-".to_owned()
    } else {
        String::new()
    };
    cleaned.extend(text.chars().filter(|character| *character != '_'));
    match direct(&cleaned) {
        Direct::Integer(value) => Some(value),
        _ => None,
    }
}

fn leading_zeros(text: &str) -> Option<&str> {
    // Match pydantic-core 2.41.5's clean_int_str before underscore cleanup.
    // This ordering also accepts its unusual "0__1" and "00-1" spellings.
    match text.as_bytes().first()? {
        b'1'..=b'9' | b'-' => return Some(text),
        b'0' => (),
        _ => return None,
    }
    for (index, byte) in text.bytes().enumerate().skip(1) {
        match byte {
            b'0' | b'_' => (),
            b'1'..=b'9' | b'-' => return Some(&text[index..]),
            b'.' => return Some(&text[index - 1..]),
            _ => return None,
        }
    }
    Some(&text[text.len() - 1..])
}

enum Direct {
    Integer(BigInt),
    TooLong,
    Clean,
}

fn direct(text: &str) -> Direct {
    let digits = text.strip_prefix('-').unwrap_or(text);
    let Some(first) = digits.as_bytes().first() else {
        return Direct::Clean;
    };
    if !first.is_ascii_digit() || (*first == b'0' && digits.len() != 1) {
        return Direct::Clean;
    }
    let length = digits.bytes().take_while(u8::is_ascii_digit).count();
    if length + usize::from(text.starts_with('-')) > MAX_DECIMAL_DIGITS {
        return Direct::TooLong;
    }
    if length != digits.len() {
        return Direct::Clean;
    }
    BigInt::parse_bytes(text.as_bytes(), 10).map(Direct::Integer).unwrap_or(Direct::Clean)
}

#[cfg(test)]
mod tests;
