//! Parse plugin identities, version constraints, and named repository scopes.

use super::config::valid_repo_name;
use crate::error::{Error, Result};
use crate::util::python_repr::string_repr;

mod host;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub name: String,
    pub spec: String,
    pub host: Option<String>,
    pub repo: Option<String>,
}

pub fn normalize_host(host: &str) -> Result<String> {
    if host::matches(host) {
        return host::normalize(host);
    }
    let url = url::Url::parse(host).map_err(|e| Error::Other(e.to_string()))?;
    let Some((scheme, rest)) = host.split_once("://").filter(|_| url.host_str().is_some()) else {
        return Err(Error::Other("plugin host must be an absolute URL with an authority".into()));
    };
    // Keep explicit ports in the identity, including default ports. URL parsers
    // may remove those when serializing a URL for transport.
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let path = url.path().strip_suffix('/').unwrap_or(url.path());
    Ok(format!("{scheme}://{authority}{path}").to_lowercase())
}

pub fn parse_reference(input: &str) -> Result<Reference> {
    if input.is_empty() {
        return Err(Error::Other("plugin reference is empty".into()));
    }
    if host::is_direct_github(input) {
        return Err(Error::Other(format!(
            "value is a GitHub URL, not a plugin reference: {}",
            string_repr(input),
        )));
    }
    let (value, repo) = split_repository(input);
    let (remaining, host) = match value.rsplit_once('@') {
        Some((left, right)) => {
            if !host::matches(right) {
                return Err(Error::Other(format!(
                    "plugin reference has an '@' but the suffix is not a valid plugin repository URL: {}",
                    string_repr(value),
                )));
            }
            (left, Some(host::normalize(right)?))
        }
        None => (value, None),
    };
    if remaining.contains('@') {
        return Err(Error::Other(format!(
            "plugin reference name/version must not contain '@': {}",
            string_repr(value),
        )));
    }
    let split = remaining.find(['=', '<', '>', '!', '~']).unwrap_or(remaining.len());
    let (name, spec) = remaining.split_at(split);
    if !spec.is_empty() && spec.chars().nth(1) != Some('=') {
        return Err(Error::Other(format!(
            "invalid plugin version spec: {}",
            string_repr(remaining)
        )));
    }
    if name.is_empty() {
        return Err(Error::Other(format!(
            "plugin reference has empty name: {}",
            string_repr(value)
        )));
    }
    if name.contains('/') {
        return Err(Error::Other(format!(
            "plugin reference name must not contain '/': {}",
            string_repr(name)
        )));
    }
    Ok(Reference {
        name: name.to_owned(),
        spec: spec.to_owned(),
        host,
        repo,
    })
}

fn split_repository(input: &str) -> (&str, Option<String>) {
    if let Some((repo, rest)) = input.split_once('/') {
        // Python's .+ excludes LF, while its $ accepts one terminal LF.
        let rest = rest.strip_suffix('\n').unwrap_or(rest);
        if valid_repo_name(repo) && !rest.is_empty() && !rest.contains('\n') {
            return (rest, Some(repo.into()));
        }
    }
    (input, None)
}
