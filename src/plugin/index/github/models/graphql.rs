//! Convert GraphQL records before applying the canonical model validators.

use serde_json::{Value, json};

use crate::error::{Error, Result};

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
        Ok(serde_json::from_value(json!({
            "commit_hash": required(value, "oid")?,
            "committed_date": required(value, "committedDate")?,
            "zipball_url": required(value, "zipballUrl")?,
        }))?)
    }
}

impl Release {
    fn from_graphql(value: &Value) -> Result<Self> {
        let empty = json!({});
        let connection = value.get("releaseAssets").unwrap_or(&empty);
        let connection = connection.as_object().ok_or_else(invalid_collection)?;
        let assets = match connection.get("nodes") {
            Some(nodes) => collect(nodes, |value| Asset::try_from(value.clone()))?,
            None => Vec::new(),
        };
        let empty_name = json!("");
        let tag_name = value.get("tagName").unwrap_or(&empty_name);
        let name = value.get("name").filter(|value| super::truthy(value));
        let target = tag_target(required(value, "tag")?)?;
        Ok(serde_json::from_value(json!({
            "name": name.unwrap_or(tag_name),
            "tag_name": tag_name,
            "commit_hash": required(target, "oid")?,
            "created_at": required(value, "createdAt")?,
            "published_at": required(value, "publishedAt")?,
            "is_prerelease": required(value, "isPrerelease")?,
            "is_draft": required(value, "isDraft")?,
            "url": required(value, "url")?,
            "zipball_url": required(target, "zipballUrl")?,
            "assets": assets,
        }))?)
    }
}

impl Tag {
    fn from_graphql(value: &Value) -> Result<Self> {
        let target = tag_target(value)?;
        Ok(serde_json::from_value(json!({
            "tag_name": required(value, "name")?,
            "commit_hash": required(target, "oid")?,
            "zipball_url": required(target, "zipballUrl")?,
            "committed_date": required(target, "committedDate")?,
        }))?)
    }
}

fn tag_target(value: &Value) -> Result<&Value> {
    let target = required(value, "target")?;
    // The source unwraps exactly one annotated tag and gives it precedence
    // over any commit fields present on the outer object.
    Ok(target.get("target").unwrap_or(target))
}

fn required<'a>(value: &'a Value, name: &str) -> Result<&'a Value> {
    value.get(name).ok_or_else(|| Error::Other(format!("GitHub record is missing field {name}")))
}

fn collect<T>(value: &Value, parse: impl Fn(&Value) -> Result<T>) -> Result<Vec<T>> {
    match value {
        Value::Array(values) => values.iter().map(parse).collect(),
        // Python comprehensions accept empty iterable objects and strings.
        Value::Object(values) if values.is_empty() => Ok(Vec::new()),
        Value::String(value) if value.is_empty() => Ok(Vec::new()),
        _ => Err(invalid_collection()),
    }
}

fn invalid_collection() -> Error {
    Error::Other("invalid GitHub metadata collection".into())
}
