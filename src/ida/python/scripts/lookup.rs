//! Typed results and framing for the interpreter-side metadata probe.

use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::time::Duration;

use serde::Deserialize;

use super::execution;
use crate::error::{Error, Result};
use crate::util::strings::python_trim;

pub(super) const SOURCE: &str = include_str!("lookup.py");

#[derive(Debug, Deserialize)]
pub(super) struct EntryPoint {
    value: String,
    distribution: Option<String>,
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ScriptInfo {
    name: String,
    pub path: Option<PathBuf>,
    #[serde(default)]
    scripts_dirs: Vec<PathBuf>,
    pub entry_point: Option<EntryPoint>,
}

impl ScriptInfo {
    pub fn not_found_message(&self) -> String {
        let searched = self
            .scripts_dirs
            .iter()
            .map(|path| format!("  {}", path.display()))
            .collect::<Vec<_>>()
            .join("\n");
        if let Some(entry) = &self.entry_point {
            let name = entry.distribution.as_deref().filter(|name| !name.is_empty()).unwrap_or("?");
            let distribution = match entry.version.as_deref().filter(|version| !version.is_empty())
            {
                Some(version) => format!("{name} {version}"),
                None => name.into(),
            };
            let reinstall = entry
                .distribution
                .as_deref()
                .filter(|name| !name.is_empty())
                .unwrap_or(&distribution);
            format!(
                "script '{}' is declared by {distribution} ({}), but no program for it was installed.\n\
                 Searched that distribution's file list and:\n{searched}\nTry reinstalling {reinstall}.",
                self.name, entry.value,
            )
        } else {
            format!("script '{}' is not installed. Searched:\n{searched}", self.name)
        }
    }
}

pub(super) async fn inspect(python: &Path, name: &str) -> Result<ScriptInfo> {
    let mut command = execution::command(python, python);
    command.args(["-c", SOURCE, name]).stdin(Stdio::inherit());
    let result = tokio::time::timeout(Duration::from_secs(60), command.output())
        .await
        .map_err(|_| {
            Error::Other(format!("failed to run {}: probe timed out after 60 s", python.display()))
        })?
        .map_err(|error| Error::Other(format!("failed to run {}: {error}", python.display())))?;
    parse(python, &result)
}

pub(super) fn parse(python: &Path, result: &Output) -> Result<ScriptInfo> {
    if !result.status.success() {
        return Err(Error::Other(format!(
            "{} exited with status {}: {}",
            python.display(),
            execution::exit_code(result.status),
            python_trim(&decode_text(&result.stderr)),
        )));
    }
    let text = decode_text(&result.stdout);
    for line in text.split(|c| {
        matches!(
            c,
            '\n' | '\r' | '\x0b' | '\x0c' | '\x1c'..='\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}'
        )
    }) {
        if let Some(document) = line.strip_prefix("__hcli__:") {
            return Ok(serde_json::from_str(document)?);
        }
    }
    Err(Error::Other(format!("no result from {}: {}", python.display(), python_trim(&text))))
}

fn decode_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace("\r\n", "\n").replace('\r', "\n")
}
