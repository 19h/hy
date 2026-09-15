//! Surrogatepass decoding for a disposable JSON syntax-validation copy.

use std::borrow::Cow;

use crate::error::Result;

use super::{Encoding, detect, invalid};

pub(crate) fn decode(bytes: &[u8]) -> Result<Cow<'_, str>> {
    let (encoding, bytes) = detect(bytes);
    match encoding {
        Encoding::Utf8 => decode_utf8(bytes),
        Encoding::Utf16Le | Encoding::Utf16Be => {
            let (chunks, remainder) = bytes.as_chunks::<2>();
            if !remainder.is_empty() {
                return Err(invalid("truncated UTF-16 code unit"));
            }
            let mut decoded = String::with_capacity(bytes.len());
            for &pair in chunks {
                let value = match encoding {
                    Encoding::Utf16Le => u16::from_le_bytes(pair),
                    _ => u16::from_be_bytes(pair),
                };
                push_codepoint(&mut decoded, u32::from(value))?;
            }
            Ok(Cow::Owned(decoded))
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
                push_codepoint(&mut decoded, value)?;
            }
            Ok(Cow::Owned(decoded))
        }
    }
}

fn decode_utf8(mut bytes: &[u8]) -> Result<Cow<'_, str>> {
    if let Ok(decoded) = std::str::from_utf8(bytes) {
        return Ok(Cow::Borrowed(decoded));
    }
    let mut decoded = String::with_capacity(bytes.len());
    while !bytes.is_empty() {
        let error = match std::str::from_utf8(bytes) {
            Ok(tail) => {
                decoded.push_str(tail);
                break;
            }
            Err(error) => error,
        };
        let (valid, tail) = bytes.split_at(error.valid_up_to());
        decoded.push_str(std::str::from_utf8(valid).expect("validated UTF-8 prefix"));
        match tail {
            [0xed, 0xa0..=0xbf, 0x80..=0xbf, rest @ ..] => {
                decoded.push(char::REPLACEMENT_CHARACTER);
                bytes = rest;
            }
            _ => return Err(invalid("invalid UTF-8 byte sequence")),
        }
    }
    Ok(Cow::Owned(decoded))
}

fn push_codepoint(decoded: &mut String, value: u32) -> Result<()> {
    // Surrogates are legal with Python's surrogatepass. Their identity cannot
    // change JSON syntax; substitute only in this disposable validation copy.
    // UTF-16 pairs need not be combined because neither unit is punctuation.
    let character = if (0xd800..=0xdfff).contains(&value) {
        char::REPLACEMENT_CHARACTER
    } else {
        char::from_u32(value).ok_or_else(|| invalid("code point exceeds U+10FFFF"))?
    };
    decoded.push(character);
    Ok(())
}
