//! Preserve urllib's repository/tag spelling after direct-install recognition.

use crate::error::{Error, Result};
use crate::plugin::index::is_direct_github;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct ReleaseSource {
    pub owner: String,
    pub repository: String,
    pub tag: Option<String>,
}

pub(super) fn parse(value: &str) -> Result<ReleaseSource> {
    let invalid = || Error::Other(format!("invalid GitHub URL: {value}"));
    if !is_direct_github(value) {
        return Err(invalid());
    }
    // Upstream splits the tag before URL parsing and removes trailing slashes
    // from that raw tag. A terminal LF therefore remains part of a tag.
    let (url, tag) = match value.rsplit_once('@') {
        Some((url, tag)) => {
            let tag = tag.trim_end_matches('/');
            if tag.is_empty() {
                return Err(invalid());
            }
            (url, Some(tag.to_owned()))
        }
        None => (value, None),
    };
    // The recognition grammar excludes queries, fragments, ports and URL params.
    // Its only accepted URL control character is a terminal LF, which urlparse
    // removes. Avoid WHATWG parsing: it normalizes dot segments and Unicode paths.
    let url = url.replace('\n', "");
    let (scheme, rest) = url.split_once("://").ok_or_else(invalid)?;
    let (authority, path) = rest.split_once('/').ok_or_else(invalid)?;
    if !scheme.eq_ignore_ascii_case("https") || !authority.eq_ignore_ascii_case("github.com") {
        return Err(invalid());
    }
    // A suffix is removed before empty path components are discarded. Thus
    // repo.git/ retains .git, while repo.git.git loses exactly one suffix.
    let path = path.strip_suffix(".git").unwrap_or(path);
    let mut parts = path.split('/').filter(|part| !part.is_empty());
    let owner = parts.next().ok_or_else(invalid)?;
    let repository = parts.next().ok_or_else(invalid)?;
    if parts.next().is_some() {
        return Err(invalid());
    }
    Ok(ReleaseSource {
        owner: owner.into(),
        repository: repository.into(),
        tag,
    })
}

#[cfg(test)]
mod tests;
