//! urllib's Latin-1 header quoting and lexical URL resolution.
//!
//! The adaptation and license notice are in util/python_path/LICENSE.

use std::fmt::Write;

use crate::error::{Error, Result};
use crate::plugin::index::transport::url_parts::authority;

pub(super) fn transport_url(value: &str) -> Result<url::Url> {
    let parts = Parts::parse(value, "")?;
    if matches!(parts.scheme.as_str(), "http" | "https") && parts.authority.is_empty() {
        // WHATWG parsing would turn http:///path into a request to host "path".
        return Err(Error::GitHubUrl("<urlopen error no host given>".into()));
    }
    value.parse().map_err(|error| Error::Other(format!("invalid GitHub redirect URL: {error}")))
}

pub(super) fn resolve(base: &str, location: &[u8]) -> Result<Option<String>> {
    let location: String = location.iter().map(|byte| char::from(*byte)).collect();
    let mut parts = Parts::parse(&location, "")?;
    if !matches!(parts.scheme.as_str(), "" | "http" | "https" | "ftp") {
        return Ok(None);
    }
    if !parts.authority.is_empty() && parts.path.is_empty() {
        parts.path = "/".into();
    }
    let mut quoted = String::new();
    for character in parts.render().chars() {
        if matches!(character, '!'..='~') {
            quoted.push(character);
        } else {
            write!(quoted, "%{:02X}", u32::from(character)).unwrap();
        }
    }
    Ok(Some(join(base, &quoted)?))
}

#[derive(Default)]
struct Parts {
    scheme: String,
    authority: String,
    path: String,
    params: String,
    query: String,
    fragment: String,
}

impl Parts {
    fn parse(value: &str, default_scheme: &str) -> Result<Self> {
        let cleaned: String = value
            .trim_start_matches(|character: char| character <= '\u{20}')
            .chars()
            .filter(|character| !matches!(character, '\t' | '\r' | '\n'))
            .collect();
        let mut parts = Self {
            scheme: default_scheme.into(),
            ..Self::default()
        };
        let mut remainder = cleaned.as_str();
        if let Some((scheme, rest)) = remainder.split_once(':')
            && scheme.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
            && scheme.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"+.-".contains(&byte))
        {
            parts.scheme = scheme.to_ascii_lowercase();
            remainder = rest;
        }
        if let Some(netloc) = remainder.strip_prefix("//") {
            let end = netloc.find(['/', '?', '#']).unwrap_or(netloc.len());
            parts.authority = netloc[..end].into();
            authority::validate(&parts.authority)
                .map_err(|error| Error::GitHubValue(error.to_string()))?;
            remainder = &netloc[end..];
        }
        if let Some((rest, fragment)) = remainder.split_once('#') {
            parts.fragment = fragment.into();
            remainder = rest;
        }
        if let Some((rest, query)) = remainder.split_once('?') {
            parts.query = query.into();
            remainder = rest;
        }
        let component_start = remainder.rfind('/').unwrap_or(0);
        if let Some(offset) = remainder[component_start..].find(';') {
            let index = component_start + offset;
            parts.params = remainder[index + 1..].into();
            remainder = &remainder[..index];
        }
        parts.path = remainder.into();
        Ok(parts)
    }

    fn render(&self) -> String {
        let mut path = self.path.clone();
        if !self.params.is_empty() {
            path.push(';');
            path.push_str(&self.params);
        }
        let mut output = if !self.authority.is_empty() {
            if !path.is_empty() && !path.starts_with('/') {
                path.insert(0, '/');
            }
            format!("//{}{path}", self.authority)
        } else if path.starts_with("//")
            || (!self.scheme.is_empty() && (path.is_empty() || path.starts_with('/')))
        {
            format!("//{path}")
        } else {
            path
        };
        if !self.scheme.is_empty() {
            output = format!("{}:{output}", self.scheme);
        }
        if !self.query.is_empty() {
            output.push('?');
            output.push_str(&self.query);
        }
        if !self.fragment.is_empty() {
            output.push('#');
            output.push_str(&self.fragment);
        }
        output
    }
}

fn join(base: &str, relative: &str) -> Result<String> {
    if relative.is_empty() {
        return Ok(base.into());
    }
    let base = Parts::parse(base, "")?;
    let mut target = Parts::parse(relative, &base.scheme)?;
    if target.scheme != base.scheme {
        return Ok(relative.into());
    }
    if !target.authority.is_empty() {
        return Ok(target.render());
    }
    target.authority = base.authority;
    if target.path.is_empty() && target.params.is_empty() {
        target.path = base.path;
        target.params = base.params;
        if target.query.is_empty() {
            target.query = base.query;
        }
    } else {
        target.path = resolve_path(&base.path, &target.path);
    }
    Ok(target.render())
}

fn resolve_path(base: &str, relative: &str) -> String {
    let segments: Vec<_> = if relative.starts_with('/') {
        relative.split('/').collect()
    } else {
        let mut base: Vec<_> = base.split('/').collect();
        if base.last() != Some(&"") {
            base.pop();
        }
        base.extend(relative.split('/'));
        let end = base.len().saturating_sub(1);
        base.into_iter()
            .enumerate()
            .filter_map(|(index, value)| {
                (index == 0 || index == end || !value.is_empty()).then_some(value)
            })
            .collect()
    };
    let mut resolved = Vec::new();
    for segment in &segments {
        match *segment {
            ".." => {
                resolved.pop();
            }
            "." => {}
            segment => resolved.push(segment),
        }
    }
    if matches!(segments.last(), Some(&"." | &"..")) {
        resolved.push("");
    }
    let path = resolved.join("/");
    if path.is_empty() {
        "/".into()
    } else {
        path
    }
}
