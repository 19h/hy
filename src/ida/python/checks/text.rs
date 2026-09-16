//! Strict UTF-8 subprocess text with CPython's universal-newline conversion.

use crate::error::Result;

pub(super) fn decode(bytes: &[u8]) -> Result<String> {
    let text = crate::util::python_utf8::decode(bytes)?;
    Ok(text.replace("\r\n", "\n").replace('\r', "\n"))
}
