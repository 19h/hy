//! GraphQL field extraction precedes canonical model validation.

use crate::error::Result;
use crate::util::python_json::Value;

use super::fields::{boolean, invalid_collection, list, required, string, text};
use super::{Asset, Commit, Release, Repository, Tag};

impl Repository {
    pub(crate) fn from_graphql(value: &Value) -> Result<Self> {
        let releases = required(required(value, "releases")?, "nodes")?;
        let tags = required(required(value, "refs")?, "nodes")?;
        let default = required(required(value, "defaultBranchRef")?, "target")?;
        Ok(Self {
            default_branch: Commit::from_graphql(default)?,
            releases: collect(releases, Release::from_graphql)?,
            tags: collect(tags, Tag::from_graphql)?,
        })
    }
}

impl Commit {
    fn from_graphql(value: &Value) -> Result<Self> {
        Ok(Self {
            commit_hash: string(value, "oid")?,
            committed_date: string(value, "committedDate")?,
            zipball_url: string(value, "zipballUrl")?,
        })
    }
}

impl Release {
    fn from_graphql(value: &Value) -> Result<Self> {
        let assets = match value.get("releaseAssets") {
            None => Vec::new(),
            Some(Value::Object(connection)) => match connection.get("nodes") {
                Some(nodes) => collect(nodes, Asset::from_value)?,
                None => Vec::new(),
            },
            _ => return Err(invalid_collection()),
        };
        let empty_name = Value::String("".into());
        let tag_name = value.get("tagName").unwrap_or(&empty_name);
        let name = value.get("name").filter(|value| value.truthy()).unwrap_or(tag_name);
        let target = tag_target(required(value, "tag")?)?;
        Ok(Self {
            name: text(name)?,
            tag_name: text(tag_name)?,
            commit_hash: string(target, "oid")?,
            created_at: string(value, "createdAt")?,
            published_at: string(value, "publishedAt")?,
            is_prerelease: boolean(required(value, "isPrerelease")?)?,
            is_draft: boolean(required(value, "isDraft")?)?,
            url: string(value, "url")?,
            zipball_url: string(target, "zipballUrl")?,
            assets,
        })
    }
}

impl Tag {
    fn from_graphql(value: &Value) -> Result<Self> {
        let target = tag_target(value)?;
        Ok(Self {
            tag_name: string(value, "name")?,
            commit_hash: string(target, "oid")?,
            zipball_url: string(target, "zipballUrl")?,
            committed_date: string(target, "committedDate")?,
        })
    }
}

fn tag_target(value: &Value) -> Result<&Value> {
    let target = required(value, "target")?;
    // The source unwraps exactly one annotated tag.
    Ok(target.get("target").unwrap_or(target))
}

fn collect<T>(value: &Value, parse: impl Fn(&Value) -> Result<T>) -> Result<Vec<T>> {
    match value {
        Value::Object(values) if values.is_empty() => Ok(Vec::new()),
        Value::String(value) if value.is_empty() => Ok(Vec::new()),
        _ => list(value, parse),
    }
}
