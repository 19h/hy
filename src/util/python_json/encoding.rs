//! Decode JSON byte encodings with Python's surrogatepass error policy.

use crate::error::Result;
use crate::util::json_encoding::{Encoding, detect};

use super::invalid;

pub(super) fn decode(bytes: &[u8]) -> Result<Vec<u32>> {
    let (encoding, bytes) = detect(bytes);
    match encoding {
        Encoding::Utf8 => utf8(bytes),
        Encoding::Utf16Le | Encoding::Utf16Be => {
            let (pairs, remainder) = bytes.as_chunks::<2>();
            if !remainder.is_empty() {
                return Err(invalid("truncated UTF-16 code unit"));
            }
            let words = pairs.iter().map(|&pair| match encoding {
                Encoding::Utf16Le => u16::from_le_bytes(pair),
                _ => u16::from_be_bytes(pair),
            });
            Ok(char::decode_utf16(words)
                .map(|decoded| match decoded {
                    Ok(character) => u32::from(character),
                    Err(surrogate) => u32::from(surrogate.unpaired_surrogate()),
                })
                .collect())
        }
        Encoding::Utf32Le | Encoding::Utf32Be => {
            let (words, remainder) = bytes.as_chunks::<4>();
            if !remainder.is_empty() {
                return Err(invalid("truncated UTF-32 code unit"));
            }
            words
                .iter()
                .map(|&word| {
                    let value = match encoding {
                        Encoding::Utf32Le => u32::from_le_bytes(word),
                        _ => u32::from_be_bytes(word),
                    };
                    if value <= 0x10ffff {
                        Ok(value)
                    } else {
                        Err(invalid("code point exceeds U+10FFFF"))
                    }
                })
                .collect()
        }
    }
}

fn utf8(mut bytes: &[u8]) -> Result<Vec<u32>> {
    let mut decoded = Vec::with_capacity(bytes.len());
    while !bytes.is_empty() {
        let error = match std::str::from_utf8(bytes) {
            Ok(tail) => {
                decoded.extend(tail.chars().map(u32::from));
                break;
            }
            Err(error) => error,
        };
        let (valid, tail) = bytes.split_at(error.valid_up_to());
        decoded.extend(
            std::str::from_utf8(valid).expect("validated UTF-8 prefix").chars().map(u32::from),
        );
        match tail {
            [0xed, second @ 0xa0..=0xbf, third @ 0x80..=0xbf, rest @ ..] => {
                decoded.push(0xd000 | (u32::from(second & 0x3f) << 6) | u32::from(third & 0x3f));
                bytes = rest;
            }
            _ => return Err(invalid("invalid UTF-8 byte sequence")),
        }
    }
    Ok(decoded)
}
