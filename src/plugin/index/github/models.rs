//! Validated Python model values, including strings with unpaired surrogates.

use crate::util::{pydantic_integer::Integer, python_json::Text};
#[cfg(test)]
use serde::Serialize;

mod cached;
mod fields;
mod graphql;
mod serialization;

#[derive(Debug)]
#[cfg_attr(test, derive(Serialize))]
pub struct Repository {
    pub default_branch: Commit,
    pub releases: Vec<Release>,
    pub tags: Vec<Tag>,
}

#[derive(Debug)]
#[cfg_attr(test, derive(Serialize))]
pub struct Release {
    pub name: Text,
    pub tag_name: Text,
    pub commit_hash: Text,
    pub created_at: Text,
    pub published_at: Text,
    pub is_prerelease: bool,
    pub is_draft: bool,
    pub url: Text,
    pub zipball_url: Text,
    pub assets: Vec<Asset>,
}

#[derive(Debug)]
#[cfg_attr(test, derive(Serialize))]
pub struct Asset {
    pub name: Text,
    pub download_url: Text,
    pub size: Integer,
    pub content_type: Text,
}

#[derive(Debug)]
#[cfg_attr(test, derive(Serialize))]
pub struct Tag {
    pub tag_name: Text,
    pub commit_hash: Text,
    pub zipball_url: Text,
    pub committed_date: Text,
}

#[derive(Debug)]
#[cfg_attr(test, derive(Serialize))]
pub struct Commit {
    pub commit_hash: Text,
    pub committed_date: Text,
    pub zipball_url: Text,
}

#[cfg(test)]
mod tests;
