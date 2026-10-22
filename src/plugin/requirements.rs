//! Validated plugin references and PEP 440 Python compatibility declarations.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Requirement {
    pub name: String,
    pub spec: String,
    pub host: Option<String>,
    pub required: bool,
}

impl Requirement {
    pub fn parse(value: &str) -> Result<Self> {
        let reference = super::index::parse_reference(value)?;
        if !super::valid_specification(&reference.spec) && !reference.spec.is_empty() {
            return Err(Error::PluginInstall(format!(
                "invalid plugin version specification: {value}"
            )));
        }
        Ok(Self {
            name: reference.name,
            spec: reference.spec,
            host: reference.host,
            required: true,
        })
    }

    pub fn matches(&self, version: &str) -> bool {
        self.spec.is_empty() || super::version_matches(version, &self.spec)
    }
}

impl std::fmt::Display for Requirement {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(output, "{}{}", self.name, self.spec)?;
        if let Some(host) = &self.host {
            write!(output, "@{host}")?;
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for Requirement {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Entry {
            Name(String),
            Options {
                plugin: String,
                #[serde(default = "required")]
                required: bool,
            },
        }
        fn required() -> bool {
            true
        }
        let (value, required) = match Entry::deserialize(deserializer)? {
            Entry::Name(value) => (value, true),
            Entry::Options {
                plugin,
                required,
            } => (plugin, required),
        };
        let reference = super::index::parse_reference(&value).map_err(serde::de::Error::custom)?;
        if reference.repo.is_some()
            || (!reference.spec.is_empty() && !reference.spec.starts_with("=="))
        {
            return Err(serde::de::Error::custom(
                "dependencies accept names and == pins without repository prefixes",
            ));
        }
        super::validate_name(&reference.name).map_err(serde::de::Error::custom)?;
        let mut requirement = Self::parse(&value).map_err(serde::de::Error::custom)?;
        requirement.required = required;
        Ok(requirement)
    }
}

impl Serialize for Requirement {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        if self.required {
            serializer.serialize_str(&self.to_string())
        } else {
            serde_json::json!({"plugin": self.to_string(), "required": false}).serialize(serializer)
        }
    }
}

pub(super) fn deserialize_python<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error> {
    let Some(value) = Option::<String>::deserialize(deserializer)? else {
        return Ok(None);
    };
    let value = crate::util::strings::python_trim(&value);
    if value.is_empty() || PythonRequirement::parse(value).is_none() {
        return Err(serde::de::Error::custom(
            "requiresPython must be a nonempty PEP 440 version specifier",
        ));
    }
    Ok(Some(value.into()))
}

pub fn python_matches(version: &str, requirement: &str) -> bool {
    let Ok(version) = version.parse::<pep440_rs::Version>() else {
        return false;
    };
    let Some(requirement) = PythonRequirement::parse(requirement) else {
        return false;
    };
    requirement.specifiers.contains(&version)
        && requirement
            .arbitrary
            .iter()
            .all(|expected| expected == &version.to_string().to_lowercase())
}

/// PEP 440's arbitrary equality compares spelling rather than parsed versions.
/// Keep those clauses separate because pep440_rs requires a parsed operand.
struct PythonRequirement {
    specifiers: pep440_rs::VersionSpecifiers,
    arbitrary: Vec<String>,
}

impl PythonRequirement {
    fn parse(value: &str) -> Option<Self> {
        let mut specifiers = Vec::new();
        let mut arbitrary = Vec::new();
        for clause in value.split(',').map(crate::util::strings::python_trim) {
            if clause.is_empty() {
                continue;
            }
            if let Some(version) = clause.strip_prefix("===") {
                let version = crate::util::strings::python_trim(version);
                if version.chars().any(|character| {
                    crate::util::strings::python_whitespace(character)
                        || matches!(character, ';' | ')')
                }) {
                    return None;
                }
                arbitrary.push(version.to_lowercase());
            } else {
                let normalized: String = clause
                    .chars()
                    .map(|character| {
                        if crate::util::strings::python_whitespace(character) {
                            ' '
                        } else {
                            character
                        }
                    })
                    .collect();
                specifiers.push(normalized.parse::<pep440_rs::VersionSpecifier>().ok()?);
            }
        }
        Some(Self {
            specifiers: specifiers.into_iter().collect(),
            arbitrary,
        })
    }
}

pub fn collect(metadata: &super::PluginMetadata) -> Vec<Requirement> {
    let mut requirements = metadata.dependencies.clone();
    for component in &metadata.components {
        if let super::components::Component::Inline(descriptor) = component {
            requirements.extend(collect(&descriptor.metadata));
        }
    }
    requirements
}

#[cfg(test)]
mod tests;
