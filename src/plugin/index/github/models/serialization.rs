//! Canonical Python model_dump fields for sorted ASCII cache publication.

use crate::error::Result;
use crate::util::{
    json_format, json_numbers,
    python_json::{Text, Value},
};

use super::{Asset, Commit, Release, Repository, Tag};

impl Repository {
    pub(crate) fn cache_text(&self) -> Result<String> {
        let value = Value::object([
            ("default_branch", self.default_branch.cache_value()),
            ("releases", Value::Array(self.releases.iter().map(Release::cache_value).collect())),
            ("tags", Value::Array(self.tags.iter().map(Tag::cache_value).collect())),
        ]);
        let text = json_format::python_sorted_ascii(&value, "  ");
        json_numbers::validate_integer_limits(&text)?;
        Ok(text)
    }
}

impl Commit {
    fn cache_value(&self) -> Value {
        Value::object([
            ("commit_hash", text(&self.commit_hash)),
            ("committed_date", text(&self.committed_date)),
            ("zipball_url", text(&self.zipball_url)),
        ])
    }
}

impl Release {
    fn cache_value(&self) -> Value {
        Value::object([
            ("name", text(&self.name)),
            ("tag_name", text(&self.tag_name)),
            ("commit_hash", text(&self.commit_hash)),
            ("created_at", text(&self.created_at)),
            ("published_at", text(&self.published_at)),
            ("is_prerelease", Value::Bool(self.is_prerelease)),
            ("is_draft", Value::Bool(self.is_draft)),
            ("url", text(&self.url)),
            ("zipball_url", text(&self.zipball_url)),
            ("assets", Value::Array(self.assets.iter().map(Asset::cache_value).collect())),
        ])
    }
}

impl Asset {
    fn cache_value(&self) -> Value {
        Value::object([
            ("name", text(&self.name)),
            ("content_type", text(&self.content_type)),
            ("size", self.size.to_python()),
            ("download_url", text(&self.download_url)),
        ])
    }
}

impl Tag {
    fn cache_value(&self) -> Value {
        Value::object([
            ("tag_name", text(&self.tag_name)),
            ("commit_hash", text(&self.commit_hash)),
            ("zipball_url", text(&self.zipball_url)),
            ("committed_date", text(&self.committed_date)),
        ])
    }
}

fn text(value: &Text) -> Value {
    Value::String(value.clone())
}
