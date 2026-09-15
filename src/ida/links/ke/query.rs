//! Parse KE request fields and preserve the remaining navigation query verbatim.

use crate::error::{Error, Result};
use crate::ida::links::{
    content_sha, filename,
    uri::{ParsedLink, normalize_controls},
};

pub(super) struct Request {
    pub name: String,
    pub content: url::Url,
    pub sha: String,
    pub navigation: Option<String>,
}

impl Request {
    pub fn parse(original: &str, parsed: &ParsedLink<'_>) -> Result<Self> {
        // Upstream constructs a dictionary from parse_qsl: the final url value wins.
        let mut content = None;
        let mut navigate = false;
        for (key, value) in url::form_urlencoded::parse(parsed.query.as_bytes()) {
            if key == "url" {
                content = Some(value);
            }
            navigate |= matches!(key.as_ref(), "ea" | "rva" | "name" | "view");
        }
        let content = content
            .filter(|value| !value.is_empty())
            .ok_or_else(|| Error::Other("KE URL is missing the 'url' download parameter".into()))?;
        if parsed.segments.len() != 1 {
            return Err(Error::Other("KE links require exactly one filename".into()));
        }
        let name = filename(&parsed.segments[0])?;
        let content = url::Url::parse(&content).map_err(|error| Error::Other(error.to_string()))?;
        let sha = content_sha(&content)?;
        Ok(Self {
            name,
            content,
            sha,
            navigation: navigate.then(|| strip_download_parameter(original)),
        })
    }
}

fn strip_download_parameter(original: &str) -> String {
    // urllib's parsing cleanup and scheme folding also apply to urlunparse.
    let cleaned = normalize_controls(original);
    let (without_fragment, fragment) = cleaned.split_once('#').unwrap_or((&cleaned, ""));
    let (base, query) = without_fragment.split_once('?').unwrap_or((without_fragment, ""));
    let (_, authority_and_path) = base.split_once(':').expect("a parsed KE URI has a scheme");
    let kept = query
        .split('&')
        .filter(|pair| !pair.is_empty() && pair.split('=').next() != Some("url"))
        .collect::<Vec<_>>()
        .join("&");
    let mut result = format!("ida:{authority_and_path}");
    if !kept.is_empty() {
        result.push('?');
        result.push_str(&kept);
    }
    if !fragment.is_empty() {
        result.push('#');
        result.push_str(fragment);
    }
    result
}

#[cfg(test)]
mod tests;
