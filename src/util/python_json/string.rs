//! Unicode code points preserve Python strings that Rust String cannot represent.

use crate::error::Result;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::invalid;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Text(pub(super) Vec<u32>);

impl From<&str> for Text {
    fn from(value: &str) -> Self {
        Self(value.chars().map(u32::from).collect())
    }
}

impl From<String> for Text {
    fn from(value: String) -> Self {
        Self::from(value.as_str())
    }
}

impl Text {
    pub(crate) fn from_codepoints(points: impl IntoIterator<Item = u32>) -> Result<Self> {
        let points: Vec<_> = points.into_iter().collect();
        if points.iter().any(|&point| point > 0x10ffff) {
            return Err(invalid("string contains an invalid Unicode code point"));
        }
        Ok(Self(points))
    }

    pub(crate) fn starts_with(&self, character: char) -> bool {
        self.0.first() == Some(&u32::from(character))
    }

    pub(crate) fn equals(&self, text: &str) -> bool {
        self.codepoints().eq(text.chars().map(u32::from))
    }

    pub(crate) fn compare(&self, text: &str) -> std::cmp::Ordering {
        self.codepoints().cmp(text.chars().map(u32::from))
    }

    pub(crate) fn codepoints(&self) -> impl Iterator<Item = u32> + '_ {
        self.0.iter().copied()
    }

    pub(crate) fn characters(&self) -> impl Iterator<Item = Self> + '_ {
        self.0.iter().map(|&point| Self(vec![point]))
    }

    pub(crate) fn split_once(&self, separator: char) -> Option<(Self, Self)> {
        let index = self.0.iter().position(|&point| point == u32::from(separator))?;
        Some((Self(self.0[..index].to_vec()), Self(self.0[index + 1..].to_vec())))
    }

    pub(crate) fn contains(&self, character: char) -> bool {
        self.0.contains(&u32::from(character))
    }

    pub(crate) fn lowercase(&self) -> Self {
        let mut output = Vec::new();
        let mut run = String::new();
        for &point in &self.0 {
            if let Some(character) = char::from_u32(point) {
                run.push(character);
            } else {
                // A surrogate separates casing contexts, including final sigma.
                output.extend(run.to_lowercase().chars().map(u32::from));
                run.clear();
                output.push(point);
            }
        }
        output.extend(run.to_lowercase().chars().map(u32::from));
        Self(output)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// ZIP's suffix consists only of ASCII letters and punctuation.
    pub(crate) fn has_zip_suffix(&self) -> bool {
        self.0.len() >= 4
            && self.0[self.0.len() - 4..].iter().zip(b".zip").all(|(&point, &expected)| {
                u8::try_from(point).is_ok_and(|byte| byte.eq_ignore_ascii_case(&expected))
            })
    }

    pub(crate) fn to_utf8(&self) -> Result<String> {
        self.0
            .iter()
            .map(|&point| {
                char::from_u32(point)
                    .ok_or_else(|| invalid("string contains an unpaired surrogate"))
            })
            .collect()
    }

    /// CPython's default Unix filesystem/stdout codec maps U+DC80–U+DCFF to bytes.
    #[cfg(any(unix, test))]
    pub(crate) fn to_utf8_surrogateescape(&self) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        for point in self.codepoints() {
            if (0xdc80..=0xdcff).contains(&point) {
                bytes.push((point - 0xdc00) as u8);
            } else {
                let character = char::from_u32(point)
                    .ok_or_else(|| invalid("string contains an unencodable surrogate"))?;
                bytes.extend_from_slice(character.encode_utf8(&mut [0; 4]).as_bytes());
            }
        }
        Ok(bytes)
    }

    /// CPython's UTF-8 stderr uses backslashreplace for unencodable surrogates.
    pub(crate) fn diagnostic(&self) -> String {
        let mut result = String::new();
        for &point in &self.0 {
            if let Some(character) = char::from_u32(point) {
                result.push(character);
            } else {
                result.push_str(&format!("\\u{point:04x}"));
            }
        }
        result
    }
}

impl Serialize for Text {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let text = self.to_utf8().map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(&text)
    }
}

impl<'de> Deserialize<'de> for Text {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        // Pydantic's JSON model validator also rejects escaped unpaired surrogates.
        String::deserialize(deserializer).map(Self::from)
    }
}
