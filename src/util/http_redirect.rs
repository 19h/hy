//! Target construction for HTTPX's automatic redirect policy.
//!
//! Repository redirects use a separate, manual `URL.join` policy. These rules
//! apply only to automatic redirects in API transfers and GitHub downloads.

use reqwest::header::{self, HeaderMap};

use crate::error::{Error, Result};

use super::http_headers::TextDecoder;

pub(crate) fn target(
    current: &url::Url,
    status: u16,
    headers: &HeaderMap,
) -> Result<Option<url::Url>> {
    if !matches!(status, 301 | 302 | 303 | 307 | 308) || !headers.contains_key(header::LOCATION) {
        return Ok(None);
    }
    let decoder = TextDecoder::new(headers);
    let location = headers
        .get_all(header::LOCATION)
        .iter()
        .map(|value| decoder.decode(value))
        .collect::<Vec<_>>()
        .join(", ");
    // HTTPX rejects controls before URL parsing; WHATWG parsing removes some.
    if location.chars().count() > 65_536 {
        return Err(invalid_url("URL exceeds 65536 characters"));
    }
    if location.chars().any(|ch| ch.is_ascii_control()) {
        return Err(invalid_url("URL contains a non-printable ASCII character"));
    }
    let repaired = repair_missing_host(current, &location)?;
    let mut target = current.join(repaired.as_deref().unwrap_or(&location)).map_err(invalid_url)?;
    if target.fragment().is_none_or(str::is_empty) {
        target.set_fragment(current.fragment());
    }
    Ok(Some(target))
}

/// HTTPX fills only the host when an absolute Location omits it. In particular,
/// the original credentials and nondefault port must not survive that repair.
fn repair_missing_host(current: &url::Url, location: &str) -> Result<Option<String>> {
    let Some((scheme, remainder)) = location.split_once(':') else {
        return Ok(None);
    };
    if !scheme.starts_with(|ch: char| ch.is_ascii_alphabetic())
        || !scheme.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"+.-".contains(&byte))
    {
        return Ok(None);
    }
    let (authority, suffix) = if let Some(remainder) = remainder.strip_prefix("//") {
        let end = remainder.find(['/', '?', '#']).unwrap_or(remainder.len());
        (&remainder[..end], &remainder[end..])
    } else {
        ("", remainder)
    };
    let (userinfo, host_port) = authority
        .rsplit_once('@')
        .map_or(("", authority), |(userinfo, host_port)| (userinfo, host_port));
    if !host_port.is_empty() && !host_port.starts_with(':') {
        return Ok(None);
    }
    // HTTPX normalizes literal dot segments before copy_with(host=...). A
    // spelling such as `http:a/..//next` therefore has the valid path `/next`.
    let path_end = suffix.find(['?', '#']).unwrap_or(suffix.len());
    let path = normalize_path(&suffix[..path_end]);
    if !path.is_empty() && !path.starts_with('/') {
        return Err(invalid_url("absolute URL path must be empty or begin with '/'"));
    }
    let host = current.host_str().ok_or_else(|| invalid_url("current URL has no host"))?;
    let credentials = if userinfo.is_empty() {
        String::new()
    } else {
        format!("{userinfo}@")
    };
    let query_fragment = &suffix[path_end..];
    Ok(Some(format!("{scheme}://{credentials}{host}{host_port}{path}{query_fragment}")))
}

fn normalize_path(path: &str) -> String {
    let mut segments = Vec::new();
    for segment in path.split('/') {
        match segment {
            "." => {}
            ".." => {
                if segments.as_slice() != [""] {
                    segments.pop();
                }
            }
            _ => segments.push(segment),
        }
    }
    segments.join("/")
}

fn invalid_url(error: impl std::fmt::Display) -> Error {
    Error::Other(format!("invalid redirect URL: {error}"))
}

#[cfg(test)]
mod tests;
