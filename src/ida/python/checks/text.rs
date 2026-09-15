//! Strict UTF-8 subprocess text with CPython's universal-newline conversion.

use crate::error::{Error, Result};

pub(super) fn decode(bytes: &[u8]) -> Result<String> {
    let text = std::str::from_utf8(bytes).map_err(|error| {
        let start = error.valid_up_to();
        let (end, reason) = match error.error_len() {
            None => (bytes.len(), "unexpected end of data"),
            Some(length) => (
                start + length,
                if (0xc2..=0xf4).contains(&bytes[start]) {
                    "invalid continuation byte"
                } else {
                    "invalid start byte"
                },
            ),
        };
        let location = if end == start + 1 {
            format!("byte 0x{:02x} in position {start}", bytes[start])
        } else {
            format!("bytes in position {start}-{}", end - 1)
        };
        Error::UnicodeDecode(format!("'utf-8' codec can't decode {location}: {reason}"))
    })?;
    Ok(text.replace("\r\n", "\n").replace('\r', "\n"))
}
