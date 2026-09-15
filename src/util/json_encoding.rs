//! CPython JSON byte-encoding detection, independent of HTTP charset declarations.

use std::borrow::Cow;

use crate::error::{Error, Result};

mod validation;
pub(crate) use validation::decode as validation_text;

pub(super) enum Encoding {
    Utf8,
    Utf16Le,
    Utf16Be,
    Utf32Le,
    Utf32Be,
}

/// Preserve Unicode values; reject unpaired surrogates that Rust strings cannot hold.
pub(crate) fn decode(bytes: &[u8]) -> Result<Cow<'_, str>> {
    let (encoding, bytes) = detect(bytes);
    match encoding {
        Encoding::Utf8 => std::str::from_utf8(bytes).map(Cow::Borrowed).map_err(invalid),
        Encoding::Utf16Le | Encoding::Utf16Be => {
            let (chunks, remainder) = bytes.as_chunks::<2>();
            if !remainder.is_empty() {
                return Err(invalid("truncated UTF-16 code unit"));
            }
            let words: Vec<_> = chunks
                .iter()
                .map(|&pair| match encoding {
                    Encoding::Utf16Le => u16::from_le_bytes(pair),
                    _ => u16::from_be_bytes(pair),
                })
                .collect();
            String::from_utf16(&words).map(Cow::Owned).map_err(invalid)
        }
        Encoding::Utf32Le | Encoding::Utf32Be => {
            let (chunks, remainder) = bytes.as_chunks::<4>();
            if !remainder.is_empty() {
                return Err(invalid("truncated UTF-32 code unit"));
            }
            let mut decoded = String::with_capacity(bytes.len());
            for &word in chunks {
                let value = match encoding {
                    Encoding::Utf32Le => u32::from_le_bytes(word),
                    _ => u32::from_be_bytes(word),
                };
                decoded
                    .push(char::from_u32(value).ok_or_else(|| invalid("invalid Unicode scalar"))?);
            }
            Ok(Cow::Owned(decoded))
        }
    }
}

pub(super) fn detect(bytes: &[u8]) -> (Encoding, &[u8]) {
    for (bom, encoding) in [
        (b"\x00\x00\xfe\xff".as_slice(), Encoding::Utf32Be),
        (b"\xff\xfe\x00\x00".as_slice(), Encoding::Utf32Le),
        (b"\xfe\xff".as_slice(), Encoding::Utf16Be),
        (b"\xff\xfe".as_slice(), Encoding::Utf16Le),
        (b"\xef\xbb\xbf".as_slice(), Encoding::Utf8),
    ] {
        if let Some(body) = bytes.strip_prefix(bom) {
            return (encoding, body);
        }
    }
    let encoding = match bytes {
        [0, 0, _, _, ..] => Encoding::Utf32Be,
        [0, _, _, _, ..] => Encoding::Utf16Be,
        [_, 0, 0, 0, ..] => Encoding::Utf32Le,
        [_, 0, _, _, ..] => Encoding::Utf16Le,
        [0, _] => Encoding::Utf16Be,
        [_, 0] => Encoding::Utf16Le,
        _ => Encoding::Utf8,
    };
    (encoding, bytes)
}

fn invalid(reason: impl std::fmt::Display) -> Error {
    Error::Other(format!("JSON byte decoding failed: {reason}"))
}
