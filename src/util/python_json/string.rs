//! Unicode code points preserve Python strings that Rust String cannot represent.

use crate::error::Result;

use super::invalid;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Text(pub(super) Vec<u32>);

impl From<&str> for Text {
    fn from(value: &str) -> Self {
        Self(value.chars().map(u32::from).collect())
    }
}

impl Text {
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
