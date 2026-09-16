//! urllib Request parsing keeps query text and literal control characters.
//!
//! The CPython adaptation and license notice are in util/python_path/LICENSE.

use crate::error::{Error, Result};
use crate::plugin::index::transport::url_parts;
use crate::util::{python_json::Text, strings::python_whitespace};

use super::path;

pub(in crate::plugin::index::github) struct Request {
    pub(super) host: Text,
    pub(super) selector: Text,
}

impl Request {
    pub(in crate::plugin::index::github) fn parse(url: &Text) -> Result<Option<Self>> {
        let points: Vec<_> = url.codepoints().collect();
        let mut value = trim(&points);
        if value.starts_with(&[u32::from('<')]) && value.ends_with(&[u32::from('>')]) {
            value = trim(&value[1..value.len() - 1]);
        }
        if value.starts_with(&ascii("URL:")) {
            value = trim(&value[4..]);
        }
        value = before(value, '#');
        let Some(colon) = value.iter().position(|&point| point == u32::from(':')) else {
            return Ok(None);
        };
        // urllib's _splittype accepts more schemes than urlparse, but only this
        // exact case-insensitive scheme selects the local file handler.
        if colon != 4
            || !value[..colon].iter().zip(b"file").all(|(&point, &byte)| {
                u8::try_from(point).is_ok_and(|point| point.eq_ignore_ascii_case(&byte))
            })
        {
            return Ok(None);
        }
        // Request.__init__ also calls urlparse to determine origin_req_host.
        validate_components(value.iter().copied())?;

        let remainder = &value[colon + 1..];
        let (host, selector) = if remainder.starts_with(&ascii("//")) {
            let remainder = &remainder[2..];
            let end = remainder
                .iter()
                .position(|&point| matches!(point, 0x2f | 0x23 | 0x3f))
                .unwrap_or(remainder.len());
            let mut selector = remainder[end..].to_vec();
            if !selector.is_empty() && selector[0] != u32::from('/') {
                selector.insert(0, u32::from('/'));
            }
            (path::unquote(&remainder[..end]), selector)
        } else {
            (Vec::new(), remainder.to_vec())
        };
        Ok(Some(Self {
            host: Text::from_codepoints(host)?,
            selector: Text::from_codepoints(selector)?,
        }))
    }

    pub(super) fn has_remote_double_slash(&self) -> bool {
        let points: Vec<_> = self.selector.codepoints().take(3).collect();
        points.starts_with(&ascii("//"))
            && points.get(2) != Some(&u32::from('/'))
            && !self.host.is_empty()
            && !self.host.equals("localhost")
    }

    /// mimetypes.guess_type parses the selector independently of the full URL.
    pub(super) fn validate_mime_selector(&self) -> Result<()> {
        validate_components(self.selector.codepoints())
    }
}

fn validate_components(points: impl Iterator<Item = u32>) -> Result<()> {
    // Surrogates have no NFKC mapping. This scalar view preserves delimiter and
    // IP-validation decisions; the retained request components remain unchanged.
    let validation: String =
        points.map(|point| char::from_u32(point).unwrap_or(char::REPLACEMENT_CHARACTER)).collect();
    url_parts::Parts::parse(&validation)
        .map(|_| ())
        .map_err(|error| Error::GitHubValue(error.to_string()))
}

fn trim(mut value: &[u32]) -> &[u32] {
    let whitespace = |point| char::from_u32(point).is_some_and(python_whitespace);
    while value.first().is_some_and(|&point| whitespace(point)) {
        value = &value[1..];
    }
    while value.last().is_some_and(|&point| whitespace(point)) {
        value = &value[..value.len() - 1];
    }
    value
}

fn before(value: &[u32], delimiter: char) -> &[u32] {
    &value[..value.iter().position(|&point| point == u32::from(delimiter)).unwrap_or(value.len())]
}

fn ascii(value: &str) -> Vec<u32> {
    value.bytes().map(u32::from).collect()
}
