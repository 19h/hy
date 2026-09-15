//! JSON container behavior at the discovery boundary.

use std::collections::BTreeSet;

use serde_json::Value;

use crate::error::{Error, Result};

use super::super::models::truthy;

pub(super) fn cached(value: &Value) -> Result<BTreeSet<String>> {
    // Upstream applies set() to the decoded root without validating a list model.
    match value {
        Value::Object(fields) => Ok(fields.keys().cloned().collect()),
        Value::String(text) => Ok(text.chars().map(|character| character.to_string()).collect()),
        Value::Array(values) => values
            .iter()
            .map(|value| {
                value.as_str().map(str::to_owned).ok_or_else(|| {
                    Error::Other("GitHub candidate cache contains a non-string repository".into())
                })
            })
            .collect(),
        _ => Err(Error::Other("GitHub candidate cache is not iterable".into())),
    }
}

#[derive(Default)]
pub(super) struct Search {
    names: BTreeSet<String>,
    has_non_string: bool,
}

impl Search {
    pub(super) fn append(&mut self, response: &Value) -> Result<usize> {
        let fields = response
            .as_object()
            .ok_or_else(|| Error::Other("GitHub search response must be an object".into()))?;
        let Some(items) = fields.get("items").filter(|value| truthy(value)) else {
            return Ok(0);
        };
        let items =
            items.as_array().ok_or_else(|| Error::Other("invalid GitHub search items".into()))?;
        for item in items {
            let name = item
                .get("repository")
                .and_then(|repository| repository.get("full_name"))
                .ok_or_else(|| Error::Other("missing GitHub search repository name".into()))?;
            match name {
                Value::String(name) => {
                    self.names.insert(name.clone());
                }
                Value::Array(_) | Value::Object(_) => {
                    return Err(Error::Other("unhashable GitHub search repository name".into()));
                }
                // Python's set accepts these scalars. Sorting or lowercasing
                // rejects them only after all search queries/pages finish.
                _ => self.has_non_string = true,
            }
        }
        Ok(items.len())
    }

    pub(super) fn finish(self) -> Result<BTreeSet<String>> {
        if self.has_non_string {
            return Err(Error::Other("GitHub search contains a non-string repository".into()));
        }
        Ok(self.names)
    }
}
