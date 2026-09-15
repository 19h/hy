//! Aliased release queries and GraphQL envelope handling before model decoding.

use std::fmt::Write;

use serde_json::{Value, json};

use crate::error::{Error, Result};
use crate::util::python_repr;

use super::models::Repository;

pub(super) fn request(repositories: &[String]) -> Result<Value> {
    let mut query = String::from("query($first: Int!) {\n");
    for (index, repository) in repositories.iter().enumerate() {
        let (owner, name) = repository
            .split_once('/')
            .ok_or_else(|| Error::Other(format!("invalid GitHub repository: {repository}")))?;
        writeln!(
            query,
            "repo{index}: repository(owner: {}, name: {}) {{",
            json!(owner),
            json!(name)
        )
        .expect("writing to String cannot fail");
        query.push_str(include_str!("releases.graphql"));
        query.push_str("\n}\n");
    }
    query.push('}');
    Ok(json!({"query":query, "variables":{"first":10}}))
}

pub(super) fn decode(
    repositories: &[String],
    response: Value,
) -> Result<Vec<(String, Repository)>> {
    check_errors(&response)?;
    let data = response
        .get("data")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::Other("GitHub GraphQL response data must be an object".into()))?;
    let mut releases = Vec::new();
    for (index, name) in repositories.iter().enumerate() {
        let Some(value) = data.get(&format!("repo{index}")).filter(|value| truthy(value)) else {
            tracing::warn!("Repository {name} not found");
            continue;
        };
        let fields = value
            .as_object()
            .ok_or_else(|| Error::Other(format!("GitHub repository {name} must be an object")))?;
        if !fields.get("defaultBranchRef").is_some_and(truthy) {
            tracing::warn!("Repository {name} not found");
            continue;
        }
        // Finish decoding the entire batch before callers publish any entries.
        let repository = serde_json::from_value(value.clone())?;
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
        let fields = error
            .as_object()
            .ok_or_else(|| Error::Other("invalid GitHub GraphQL error record".into()))?;
        if fields.get("type").and_then(Value::as_str) != Some("NOT_FOUND") {
            fatal.push(error.clone());
        }
    }
    if !fatal.is_empty() {
        return Err(Error::Other(format!(
            "GraphQL errors: {}",
            python_repr::json_str(&json!(fatal))
        )));
    }
    for error in errors {
        let message = python_repr::json_str(error.get("message").unwrap_or(&Value::Null));
        tracing::warn!("GitHub GraphQL NOT_FOUND (repo deleted/renamed): {message}");
    }
    Ok(())
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64() != Some(0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
    }
}

#[cfg(test)]
mod tests;
