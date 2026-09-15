//! Supported agents, executable discovery and subprocess boundaries.

use std::path::PathBuf;
use std::process::Stdio;

use serde_json::Value;
use tokio::process::Command;

use crate::error::{Error, Result};
use crate::ida::python::find_command;

#[derive(Clone, Copy, Debug)]
pub(super) enum Kind {
    Claude,
    Codex,
    Copilot,
    Pi,
    Omp,
}

impl Kind {
    pub(super) fn command(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Copilot => "copilot",
            Self::Pi => "pi",
            Self::Omp => "omp",
        }
    }

    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Codex => "Codex CLI",
            Self::Copilot => "GitHub Copilot CLI",
            Self::Pi => "Pi",
            Self::Omp => "Oh My Pi",
        }
    }

    pub(super) fn supports_local(self) -> bool {
        matches!(self, Self::Claude | Self::Pi | Self::Omp)
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Scope {
    Local,
    Global,
}

impl Scope {
    pub(super) fn cli(self) -> &'static str {
        match self {
            Self::Local => "project",
            Self::Global => "user",
        }
    }
}

pub(super) struct Agent {
    pub(super) kind: Kind,
    pub(super) executable: PathBuf,
}

impl Agent {
    pub(super) fn discover() -> Vec<Self> {
        [Kind::Claude, Kind::Codex, Kind::Copilot, Kind::Pi, Kind::Omp]
            .into_iter()
            .filter_map(|kind| {
                let executable = find_command(kind.command())
                    .or_else(|| find_command(&format!("{}.cmd", kind.command())))?;
                Some(Self {
                    kind,
                    executable,
                })
            })
            .collect()
    }

    pub(super) fn label(&self) -> String {
        let executable = self.executable.file_name().unwrap_or_default().to_string_lossy();
        format!("{} ({executable})", self.kind.name())
    }

    pub(super) async fn query(&self, args: &[&str]) -> Result<Option<String>> {
        let result = Command::new(&self.executable)
            .args(args)
            .stdin(Stdio::inherit())
            .output()
            .await
            .map_err(|error| self.start_error(error))?;
        Ok(result.status.success().then(|| {
            // subprocess.run(text=True, errors="replace") decodes UTF-8 and
            // translates universal newlines. Query failures remain absent data.
            String::from_utf8_lossy(&result.stdout).replace("\r\n", "\n").replace('\r', "\n")
        }))
    }

    pub(super) async fn query_json(&self, args: &[&str]) -> Result<Value> {
        Ok(self
            .query(args)
            .await?
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or(Value::Null))
    }

    pub(super) async fn checked(&self, args: &[&str]) -> Result<()> {
        println!("> {} {}", self.kind.command(), args.join(" "));
        let status = Command::new(&self.executable)
            .args(args)
            .status()
            .await
            .map_err(|error| self.start_error(error))?;
        if status.success() {
            return Ok(());
        }
        #[cfg(unix)]
        let code = {
            use std::os::unix::process::ExitStatusExt;
            status.code().unwrap_or_else(|| -status.signal().unwrap_or(1))
        };
        #[cfg(not(unix))]
        let code = status.code().unwrap_or(1);
        Err(Error::Other(format!("{} exited with status {code}", self.kind.command())))
    }

    fn start_error(&self, error: std::io::Error) -> Error {
        Error::Other(format!("could not start {}: {error}", self.kind.command()))
    }
}
