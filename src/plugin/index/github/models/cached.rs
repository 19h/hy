//! Canonical cache-model validation, without a Unicode-scalar JSON intermediate.

use crate::error::Result;
use crate::util::python_json::Value;

use super::fields::{boolean, integer, list, required, string, text};
use super::{Asset, Commit, Release, Repository, Tag};

impl Repository {
    pub(crate) fn from_cached(value: &Value) -> Result<Self> {
        Ok(Self {
            default_branch: Commit::from_cached(required(value, "default_branch")?)?,
            releases: list(required(value, "releases")?, Release::from_cached)?,
            tags: list(required(value, "tags")?, Tag::from_cached)?,
        })
    }
}

impl Commit {
    fn from_cached(value: &Value) -> Result<Self> {
        Ok(Self {
            commit_hash: string(value, "commit_hash")?,
            committed_date: string(value, "committed_date")?,
            zipball_url: string(value, "zipball_url")?,
        })
    }
}

impl Release {
    fn from_cached(value: &Value) -> Result<Self> {
        Ok(Self {
            name: string(value, "name")?,
            tag_name: string(value, "tag_name")?,
            commit_hash: string(value, "commit_hash")?,
            created_at: string(value, "created_at")?,
            published_at: string(value, "published_at")?,
            is_prerelease: boolean(required(value, "is_prerelease")?)?,
            is_draft: boolean(required(value, "is_draft")?)?,
            url: string(value, "url")?,
            zipball_url: string(value, "zipball_url")?,
            assets: list(required(value, "assets")?, Asset::from_value)?,
        })
    }
}

impl Asset {
    pub(super) fn from_value(value: &Value) -> Result<Self> {
        let field = |alias, name| {
            // Alias presence wins even when its value fails validation.
            value.get(alias).map(Ok).unwrap_or_else(|| required(value, name))
        };
        Ok(Self {
            name: string(value, "name")?,
            content_type: text(field("contentType", "content_type")?)?,
            size: integer(required(value, "size")?)?,
            download_url: text(field("downloadUrl", "download_url")?)?,
        })
    }
}

impl Tag {
    fn from_cached(value: &Value) -> Result<Self> {
        Ok(Self {
            tag_name: string(value, "tag_name")?,
            commit_hash: string(value, "commit_hash")?,
            zipball_url: string(value, "zipball_url")?,
            committed_date: string(value, "committed_date")?,
        })
    }
}
