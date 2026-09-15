//! Header tokenization and date conversion precede normalization of any cookie.

use crate::util::http_headers::trim;

use super::dates;

mod normalize;
mod words;
pub(super) use normalize::Normalized;
pub(super) use words::split;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Format {
    Netscape,
    Rfc2965,
}

pub(super) struct Attribute {
    key: String,
    value: Option<String>,
    expiry: Option<i64>,
}

pub(super) fn attributes(header: &str, year: i32) -> Result<Vec<Attribute>, ()> {
    let mut attributes = Vec::new();
    for (index, part) in header.split(';').enumerate() {
        let (key, value) = pair(part);
        if key.is_empty() {
            if index == 0 {
                break;
            }
            continue;
        }
        let expiry = if index != 0 && key.eq_ignore_ascii_case("expires") {
            match value {
                Some(value) => dates::parse(unquote(value), year)?,
                None => None,
            }
        } else {
            None
        };
        let value = if index != 0 && key.eq_ignore_ascii_case("version") {
            value.map(unquote)
        } else {
            value
        };
        attributes.push(Attribute {
            key: key.into(),
            value: value.map(str::to_owned),
            expiry,
        });
    }
    Ok(attributes)
}

fn pair(part: &str) -> (&str, Option<&str>) {
    match trim(part).split_once('=') {
        Some((key, value)) => (trim(key), Some(trim(value))),
        None => (trim(part), None),
    }
}

fn unquote(value: &str) -> &str {
    let value = value.strip_prefix('"').unwrap_or(value);
    value.strip_suffix('"').unwrap_or(value)
}
