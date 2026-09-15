//! The reference parser's closed host patterns and CPython ignore-case behavior.

use once_cell::sync::Lazy;
use regex::Regex;

use crate::error::{Error, Result};
use crate::util::python_repr::string_repr;

static GITHUB: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^https://github\.com/[a-z0-9._-]+/[a-z0-9._-]+/?\z").unwrap());
static PORTAL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^https://plugins\.hex-rays\.com/[a-z0-9._-]+/[a-z0-9._-]+(?:/[a-z0-9._-]+)?/?\z")
        .unwrap()
});
static DIRECT_GITHUB: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^https://github\.com/[a-z0-9._-]+/[a-z0-9._-]+(?:\.git)?(?:@[a-z0-9._/+-]+)?/?\z")
        .unwrap()
});

pub(super) fn is_direct_github(value: &str) -> bool {
    DIRECT_GITHUB.is_match(&pattern_spelling(value))
}

pub(super) fn matches(value: &str) -> bool {
    let value = pattern_spelling(value);
    GITHUB.is_match(&value) || PORTAL.is_match(&value)
}

fn pattern_spelling(value: &str) -> String {
    // These four extra letters belong to Python's case-insensitive ASCII ranges.
    value
        .strip_suffix('\n')
        .unwrap_or(value)
        .chars()
        .map(|character| match character {
            'İ' | 'ı' => 'i',
            'ſ' => 's',
            'K' => 'k',
            other => other.to_ascii_lowercase(),
        })
        .collect()
}

/// Called only after the closed host pattern has matched.
pub(super) fn normalize(value: &str) -> Result<String> {
    let trimmed = value.strip_suffix('\n').unwrap_or(value);
    let (scheme, rest) = trimmed.split_once("://").expect("matched host pattern");
    // A long-s can match the regex's scheme, but urlparse requires ASCII there.
    if !scheme.is_ascii() {
        return Err(Error::Other(format!("invalid plugin host URL: {}", string_repr(value))));
    }
    Ok(format!("{scheme}://{}", rest.strip_suffix('/').unwrap_or(rest)).to_lowercase())
}
