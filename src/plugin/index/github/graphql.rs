//! Aliased release queries and GraphQL envelope handling before model decoding.

use std::fmt::Write;

use serde_json::{Value as Json, json};

use crate::error::{Error, Result};
use crate::util::{python_json::Value, python_repr};

use super::models::Repository;

pub(super) fn request(repositories: &[String]) -> Result<Json> {
    let mut query = String::from("query($first: Int!) {\n");
    for (index, repository) in repositories.iter().enumerate() {
        let (owner, name) = repository
            .split_once('/')
            .ok_or_else(|| Error::Other(format!("invalid GitHub repository: {repository}")))?;
        writeln!(query, "repo{index}: repository(owner: \"{owner}\", name: \"{name}\") {{")
            .expect("writing to String cannot fail");
        query.push_str(include_str!("releases.graphql"));
        query.push_str("\n}\n");
    }
    query.push('}');
    Ok(json!({"query":query, "variables":{"first":10}}))
}

pub(super) fn decode(
    repositories: &[String],
    response: &Value,
) -> Result<Vec<(String, Repository)>> {
    check_errors(response)?;
    let Some(Value::Object(data)) = response.get("data") else {
        return Err(Error::Other("GitHub GraphQL response data must be an object".into()));
    };
    let mut releases = Vec::new();
    for (index, name) in repositories.iter().enumerate() {
        let Some(value) = data.get(&format!("repo{index}")).filter(|value| value.truthy()) else {
            tracing::warn!("Repository {name} not found");
            continue;
        };
        let Value::Object(fields) = value else {
            return Err(Error::Other(format!("GitHub repository {name} must be an object")));
        };
        if !fields.get("defaultBranchRef").is_some_and(Value::truthy) {
            tracing::warn!("Repository {name} not found");
            continue;
        }
        // Finish decoding the entire batch before callers publish any entries.
        let repository = Repository::from_graphql(value)?;
        releases.push((name.clone(), repository));
    }
    Ok(releases)
}

fn check_errors(response: &Value) -> Result<()> {
    let errors = match response.get("errors") {
        None => &[][..],
        Some(Value::Array(values)) => values,
        Some(Value::Object(values)) if values.is_empty() => &[],
        Some(Value::String(value)) if value.is_empty() => &[],
        _ => return Err(Error::Other("invalid GitHub GraphQL errors collection".into())),
    };
    let mut fatal = Vec::new();
    for error in errors {
        let Value::Object(fields) = error else {
            return Err(Error::Other("invalid GitHub GraphQL error record".into()));
        };
        if !matches!(fields.get("type"), Some(Value::String(text)) if text.equals("NOT_FOUND")) {
            fatal.push(python_repr::python_str(error));
        }
    }
    if !fatal.is_empty() {
        return Err(Error::Other(format!("GraphQL errors: [{}]", fatal.join(", "))));
    }
    for error in errors {
        let message = python_repr::python_str(error.get("message").unwrap_or(&Value::Null));
        tracing::warn!("GitHub GraphQL NOT_FOUND (repo deleted/renamed): {message}");
    }
    Ok(())
}

#[cfg(test)]
mod tests;
