//! IDA interpreter discovery, environment inspection, and subprocess execution.
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::error::{Error, Result};

mod checks;
mod create;
mod environment;
mod explain;
mod layout;
mod pip;
mod platform;
mod probe;
mod resolution;
mod scripts;
mod venv;

pub use create::{CreateOptions, create_environment};
pub use environment::doctor;
pub(crate) use environment::validate_dependency_environment;
pub use explain::explain;
pub use pip::PipOptions;
pub(crate) use pip::normalize_find_links;
pub use probe::{probe_ida, probe_installation};
pub use resolution::resolve;
pub use scripts::{find_script, resolve_for_execution, run_python, run_script};
pub use venv::install_dependencies;
pub(crate) use venv::{install_with_constraints, verify_with_constraints};

/// Full version for PEP 440 comparisons; a failed probe is memoized for this command.
pub(crate) async fn full_version() -> Result<Option<String>> {
    static VERSION: tokio::sync::OnceCell<Option<String>> = tokio::sync::OnceCell::const_new();
    Ok(VERSION
        .get_or_init(|| async {
            let python = resolve().await.ok()?;
            let output = output(
                command(&python.exe)
                    .args(["-c", "import sys; print('.'.join(map(str, sys.version_info[:3])))"]),
                10,
            )
            .await
            .ok()?;
            output
                .status
                .success()
                .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        })
        .await
        .clone())
}

pub(crate) async fn detect_current_version() -> Result<String> {
    let python = resolve().await?;
    checks::version(&python.exe).await?.ok_or_else(|| {
        Error::PythonNotFound(format!(
            "failed to probe the version of IDA's Python interpreter: {}",
            python.exe.display()
        ))
    })
}

const PROBE: &str = r#"
import importlib.util
import json
import os
import sys
import sysconfig

print('__hy__:' + json.dumps({
    'frozen': getattr(sys, 'frozen', False),
    'executable': sys.executable,
    'prefix': sys.prefix,
    'base_prefix': sys.base_prefix,
    'version': '%d.%d' % sys.version_info[:2],
    'pip_available': importlib.util.find_spec('pip') is not None,
    'externally_managed': os.path.isfile(
        os.path.join(sysconfig.get_path('stdlib'), 'EXTERNALLY-MANAGED')
    ),
    'scripts': sysconfig.get_path('scripts'),
    'virtual_env': os.environ.get('VIRTUAL_ENV'),
    'idapython_venv_executable': os.environ.get('IDAPYTHON_VENV_EXECUTABLE'),
}))
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Probe {
    #[serde(default)]
    pub frozen: bool,
    pub executable: Option<String>,
    pub prefix: String,
    pub base_prefix: String,
    pub version: String,
    pub externally_managed: bool,
    #[serde(default)]
    pub virtual_env: Option<String>,
    #[serde(default)]
    pub idapython_venv_executable: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedPython {
    pub exe: PathBuf,
    pub source: String,
    pub ida_probe: Option<Probe>,
}

pub fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|s| !s.is_empty())
}

pub fn venv_python(root: &Path) -> PathBuf {
    root.join(if cfg!(windows) {
        "Scripts/python.exe"
    } else {
        "bin/python"
    })
}

pub fn find_command(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for suffix in if cfg!(windows) {
            &["", ".exe", ".cmd", ".bat"][..]
        } else {
            &[""][..]
        } {
            let candidate = dir.join(format!("{name}{suffix}"));
            if candidate.is_file() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if std::fs::metadata(&candidate).ok()?.permissions().mode() & 0o111 == 0 {
                        continue;
                    }
                }
                return Some(candidate);
            }
        }
    }
    None
}

pub fn command(exe: &Path) -> Command {
    let mut command = Command::new(exe);
    configure_environment(&mut command, exe);
    command.kill_on_drop(true);
    command
}

pub(super) fn configure_environment(command: &mut Command, exe: &Path) {
    clean_parent_environment(command);
    if let Some(root) = environment::venv_root(exe) {
        command.env("VIRTUAL_ENV", root);
    } else {
        command.env_remove("VIRTUAL_ENV");
    }
    let directory =
        exe.parent().filter(|path| !path.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let mut path = directory.as_os_str().to_owned();
    if let Some(inherited) = std::env::var_os("PATH").filter(|value| !value.is_empty()) {
        path.push(if cfg!(windows) {
            ";"
        } else {
            ":"
        });
        path.push(inherited);
    }
    command.env("PATH", path);
}

/// Do not pass an embedding interpreter's module paths to a different Python.
pub(crate) fn clean_parent_environment(command: &mut Command) {
    for name in ["PYTHONHOME", "PYTHONPATH", "PYTHONEXECUTABLE", "PYTHONSTARTUP"] {
        command.env_remove(name);
    }
}

pub async fn output(cmd: &mut Command, seconds: u64) -> Result<std::process::Output> {
    cmd.kill_on_drop(true).stdin(Stdio::null());
    tokio::time::timeout(Duration::from_secs(seconds), cmd.output())
        .await
        .map_err(|_| Error::Other(format!("subprocess timed out after {seconds} s")))?
        .map_err(Error::from)
}

fn parse_probe(bytes: &[u8]) -> Result<Probe> {
    let text = String::from_utf8_lossy(bytes);
    let value = text
        .lines()
        .find_map(|line| line.split_once("__hy__:").map(|(_, value)| value))
        .ok_or_else(|| Error::Other("interpreter did not emit probe result".into()))?;
    Ok(serde_json::from_str(value)?)
}

pub async fn inspect(exe: &Path) -> Result<Probe> {
    let result = output(command(exe).args(["-c", PROBE]), 30).await?;
    if !result.status.success() {
        return Err(Error::Other(format!(
            "Python probe failed: {}",
            String::from_utf8_lossy(&result.stderr)
        )));
    }
    parse_probe(&result.stdout)
}
