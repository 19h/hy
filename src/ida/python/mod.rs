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
pub use venv::verify_dependencies;

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

pub fn venv_root(exe: &Path) -> Option<PathBuf> {
    let parent = exe.parent()?;
    if parent.join("pyvenv.cfg").is_file() {
        return Some(parent.into());
    }
    let root = parent.parent()?;
    root.join("pyvenv.cfg").is_file().then(|| root.to_path_buf())
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
    let mut cmd = Command::new(exe);
    cmd.env_remove("PYTHONHOME").env("PYTHONUTF8", "1");
    if let Some(root) = venv_root(exe) {
        cmd.env("VIRTUAL_ENV", root);
    } else {
        cmd.env_remove("VIRTUAL_ENV");
    }
    let mut paths = vec![exe.parent().unwrap_or(Path::new(".")).to_path_buf()];
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&path));
    }
    if let Ok(path) = std::env::join_paths(paths) {
        cmd.env("PATH", path);
    }
    cmd.kill_on_drop(true);
    cmd
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
