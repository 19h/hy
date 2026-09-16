//! JSON container behavior at the discovery boundary.

use std::collections::BTreeSet;

use crate::error::{Error, Result};
use crate::util::python_json::{Text, Value};

pub(super) fn cached(value: &Value) -> Result<BTreeSet<Text>> {
    // Upstream applies set() to the decoded root without validating a list model.
    match value {
        Value::Object(fields) => Ok(fields.keys().cloned().collect()),
        Value::String(text) => Ok(text.characters().collect()),
        Value::Array(values) => values
            .iter()
            .map(|value| match value {
                Value::String(text) => Ok(text.clone()),
                _ => Err(Error::Other(
                    "GitHub candidate cache contains a non-string repository".into(),
                )),
            })
            .collect(),
        _ => Err(Error::Other("GitHub candidate cache is not iterable".into())),
    }
}

#[derive(Default)]
pub(super) struct Search {
    names: BTreeSet<Text>,
    has_non_string: bool,
}

impl Search {
    pub(super) fn append(&mut self, response: &Value) -> Result<usize> {
        let Value::Object(fields) = response else {
            return Err(Error::Other("GitHub search response must be an object".into()));
        };
        let Some(items) = fields.get("items").filter(|value| value.truthy()) else {
            return Ok(0);
        };
        let Value::Array(items) = items else {
            return Err(Error::Other("invalid GitHub search items".into()));
        };
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

    pub(super) fn finish(self) -> Result<BTreeSet<Text>> {
        if self.has_non_string {
            return Err(Error::Other("GitHub search contains a non-string repository".into()));
        }
        Ok(self.names)
    }
}

pub(super) fn encode(names: &BTreeSet<Text>) -> String {
    let mut output = String::from("[");
    for (index, name) in names.iter().enumerate() {
        output.push_str(if index == 0 {
            "\n  "
        } else {
            ",\n  "
        });
        crate::util::json_format::write_codepoints(name.codepoints(), &mut output);
    }
    if !names.is_empty() {
        output.push('\n');
    }
    output.push(']');
    output
}
