//! Resolve explicit requirements or PEP 723 metadata without executing plugin code.

use std::path::Path;

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

use super::PluginMetadata;
use crate::error::{Error, Result};

static SCRIPT_BLOCK: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?ms)#\s*///\s*script\s*\n(.*?)#\s*///\s*\n").expect("constant PEP 723 expression")
});

#[derive(Deserialize)]
struct ScriptMetadata {
    #[serde(default)]
    dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PythonDependencies {
    Requirements(Vec<String>),
    Source(String),
}

impl Default for PythonDependencies {
    fn default() -> Self {
        Self::Requirements(Vec::new())
    }
}

fn inline_entry_point(metadata: &PluginMetadata) -> Result<Option<&str>> {
    match &metadata.python_dependencies {
        PythonDependencies::Requirements(_) => Ok(None),
        PythonDependencies::Source(source) if source == "inline" => {
            let entry = Some(metadata.entry_point.as_str())
                .filter(|entry| entry.ends_with(".py"))
                .ok_or_else(|| {
                    Error::PluginInstall(
                        "inline dependencies require a Python (.py) entry point".into(),
                    )
                })?;
            Ok(Some(entry))
        }
        PythonDependencies::Source(source) => {
            Err(Error::PluginInstall(format!("unsupported Python dependency source: {source}")))
        }
    }
}

fn explicit_dependencies(metadata: &PluginMetadata) -> Vec<String> {
    match &metadata.python_dependencies {
        PythonDependencies::Requirements(requirements) => requirements.clone(),
        _ => Vec::new(),
    }
}

pub fn dependencies_from_directory(
    metadata: &PluginMetadata,
    directory: &Path,
) -> Result<Vec<String>> {
    let Some(entry) = inline_entry_point(metadata)? else {
        return Ok(explicit_dependencies(metadata));
    };
    let path = crate::util::python_path::join(directory, entry);
    parse_inline_dependencies(&std::fs::read_to_string(path)?)
}

pub fn parse_inline_dependencies(script: &str) -> Result<Vec<String>> {
    // Match upstream's first complete script block, including its closing newline.
    let Some(block) = SCRIPT_BLOCK.captures(script) else {
        return Ok(Vec::new());
    };
    let toml = block[1]
        .lines()
        .map(str::trim)
        .map(|line| line.strip_prefix('#').unwrap_or(line).trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let metadata: ScriptMetadata = toml::from_str(&toml)
        .map_err(|error| Error::PluginInstall(format!("invalid PEP 723 metadata: {error}")))?;
    Ok(metadata.dependencies)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_metadata_uses_the_first_complete_script_block() {
        let script = "# /// script\n# requires-python = '>=3.10'\n# dependencies = [\n#   'package[extra]>=1',\n#   'other; sys_platform == \"win32\"',\n# ]\n# ///\nraise RuntimeError('must not run')\n";
        assert_eq!(
            parse_inline_dependencies(script).unwrap(),
            ["package[extra]>=1", "other; sys_platform == \"win32\""]
        );
        let multiple = "# /// script\n# dependencies = ['first']\n# ///\n# /// script\n# dependencies = ['second']\n# ///\n";
        assert_eq!(parse_inline_dependencies(multiple).unwrap(), ["first"]);
        for absent in [
            "# no metadata",
            "# /// script\n# dependencies = ['ignored']\n",
            "# /// other\n# dependencies = ['ignored']\n# ///\n",
        ] {
            assert!(parse_inline_dependencies(absent).unwrap().is_empty());
        }
    }

    #[test]
    fn malformed_inline_metadata_is_an_error() {
        for metadata in ["dependencies = 'not a list'", "dependencies = [1]", "dependencies = ["] {
            assert!(
                parse_inline_dependencies(&format!("# /// script\n# {metadata}\n# ///\n")).is_err()
            );
        }
    }
}
