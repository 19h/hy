//! Adapt Python's extra number tokens before delegating JSON grammar to serde.

use crate::error::{Error, Result};

// CPython's default decimal-int conversion limit. Floats have no digit limit.
pub(super) const MAX_INTEGER_DIGITS: usize = 4300;

pub(crate) fn normalize_for_validation(document: &str) -> Result<Vec<u8>> {
    let mut bytes = document.as_bytes().to_vec();
    scan(document, |range| {
        bytes[range.start] = b'0';
        bytes[range.start + 1..range.end].fill(b' ');
    })?;
    Ok(bytes)
}

pub(crate) fn validate_integer_limits(document: &str) -> Result<()> {
    scan(document, |_| {})
}

fn scan(document: &str, mut constant: impl FnMut(std::ops::Range<usize>)) -> Result<()> {
    let bytes = document.as_bytes();
    let mut index = 0;
    let mut quoted = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if quoted {
            match byte {
                b'\\' => index += 1,
                b'"' => quoted = false,
                _ => {}
            }
        } else if byte == b'"' {
            quoted = true;
        } else if let Some(length) = constant_length(bytes, index) {
            constant(index..index + length);
            index += length;
            continue;
        } else if byte.is_ascii_digit() || byte == b'-' {
            let start = index;
            while index < bytes.len()
                && matches!(bytes[index], b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E')
            {
                index += 1;
            }
            let number = &bytes[start..index];
            if !number.iter().any(|byte| matches!(byte, b'.' | b'e' | b'E'))
                && number.iter().filter(|byte| byte.is_ascii_digit()).count() > MAX_INTEGER_DIGITS
            {
                return Err(Error::Other(
                    "JSON integer exceeds Python's 4300-digit default limit".into(),
                ));
            }
            continue;
        }
        index += 1;
    }
    Ok(())
}

fn constant_length(bytes: &[u8], index: usize) -> Option<usize> {
    if index > 0 && !delimiter(bytes[index - 1]) {
        return None;
    }
    for constant in [b"NaN".as_slice(), b"Infinity".as_slice(), b"-Infinity".as_slice()] {
        let end = index + constant.len();
        if bytes[index..].starts_with(constant) && (end == bytes.len() || delimiter(bytes[end])) {
            return Some(constant.len());
        }
    }
    None
}

fn delimiter(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | b'\n' | b'[' | b']' | b'{' | b'}' | b',' | b':')
}
