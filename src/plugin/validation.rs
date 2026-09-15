//! Model invariants shared by every deserialization path.

use once_cell::sync::Lazy;
use regex::Regex;

use super::PluginMetadata;
use crate::error::{Error, Result};

static REPOSITORY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(concat!(
        r"^https://(?:github\.com/[a-zA-Z0-9._-]+/[a-zA-Z0-9._-]+",
        r"|plugins\.hex-rays\.com/[a-zA-Z0-9._-]+/[a-zA-Z0-9._-]+(?:/[a-zA-Z0-9._-]+)?)/?$"
    ))
    .expect("constant plugin identity expression")
});

static CATEGORIES: Lazy<Vec<String>> = Lazy::new(|| {
    serde_json::from_value(super::ida_plugin_json_schema()["$defs"]["PluginMetadata"]["properties"]["categories"]["items"]["enum"].clone())
        .expect("category values in the pinned plugin schema")
});

pub(super) fn validate(metadata: &PluginMetadata) -> Result<()> {
    super::validate_name(&metadata.name)?;
    if super::parse_version(&metadata.version).is_none() {
        return Err(Error::PluginInstall(format!("invalid plugin version: {}", metadata.version)));
    }
    if !REPOSITORY.is_match(&metadata.urls.repository) {
        return Err(Error::PluginInstall(
            "repository must identify a GitHub repository or Hex-Rays plugin page".into(),
        ));
    }
    if metadata.authors.is_empty() && metadata.maintainers.is_empty() {
        return Err(Error::PluginInstall("authors or maintainers must be present".into()));
    }
    for category in &metadata.categories {
        if !CATEGORIES.contains(category) {
            return Err(Error::PluginInstall(format!("unknown plugin category: {category}")));
        }
    }
    for (key, setting) in &metadata.settings {
        if setting.setting_type == super::SettingType::Boolean
            && (setting.choices.is_some() || setting.pattern.is_some() || setting.secret)
        {
            return Err(Error::PluginInstall(format!(
                "boolean setting {key} cannot have choices, a validation pattern, or secret input"
            )));
        }
        if setting.choices.as_ref().is_some_and(Vec::is_empty) {
            return Err(Error::PluginInstall(format!("choices must not be empty: {key}")));
        }
        if setting.choices.is_some() && setting.pattern.is_some() {
            return Err(Error::PluginInstall(format!(
                "choices and validation_pattern are mutually exclusive: {key}"
            )));
        }
        if !setting.prompt && setting.default.is_none() {
            return Err(Error::PluginInstall(format!("prompt=false requires a default: {key}")));
        }
        if let Some(default) = &setting.default {
            setting.validate_value(key, default)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use serde_json::Value;
    use std::collections::BTreeMap;

    use crate::plugin::PluginManifest;

    #[derive(Deserialize)]
    struct Fixture {
        base: Value,
        cases: Vec<Case>,
    }

    #[derive(Deserialize)]
    struct Case {
        name: String,
        #[serde(default)]
        changes: BTreeMap<String, Value>,
        remove: Option<String>,
        valid: bool,
        #[serde(default)]
        normalized: BTreeMap<String, Value>,
    }

    #[test]
    fn matches_pinned_upstream_manifest_validation() {
        let fixture: Fixture =
            serde_json::from_str(include_str!("../../tests/fixtures/manifest-validation.json"))
                .unwrap();
        for case in fixture.cases {
            let mut input = fixture.base.clone();
            for (path, value) in case.changes {
                *input.pointer_mut(&path).unwrap() = value;
            }
            if let Some(path) = case.remove {
                let (parent, key) = path.rsplit_once('/').unwrap();
                input.pointer_mut(parent).unwrap().as_object_mut().unwrap().remove(key);
            }
            let result = serde_json::from_value::<PluginManifest>(input);
            assert_eq!(result.is_ok(), case.valid, "{}: {result:?}", case.name);
            if let Ok(metadata) = result {
                let normalized = serde_json::to_value(metadata).unwrap();
                for (path, expected) in case.normalized {
                    assert_eq!(normalized.pointer(&path), Some(&expected), "{}: {path}", case.name);
                }
                assert!(normalized.get("$schema").is_none());
            }
        }
    }
}
