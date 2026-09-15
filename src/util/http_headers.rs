//! HTTPX's response-header text decoding and Python whitespace conventions.

use std::borrow::Cow;

use reqwest::header::{HeaderMap, HeaderValue};

pub(crate) use crate::util::strings::{python_trim as trim, python_whitespace as whitespace};

pub(crate) struct TextDecoder {
    utf8: bool,
}

impl TextDecoder {
    pub(crate) fn new(headers: &HeaderMap) -> Self {
        Self {
            utf8: headers.values().all(|value| std::str::from_utf8(value.as_bytes()).is_ok()),
        }
    }

    pub(crate) fn decode<'a>(&self, value: &'a HeaderValue) -> Cow<'a, str> {
        if self.utf8 {
            Cow::Borrowed(std::str::from_utf8(value.as_bytes()).expect("validated header UTF-8"))
        } else {
            Cow::Owned(value.as_bytes().iter().map(|byte| char::from(*byte)).collect())
        }
    }
}
