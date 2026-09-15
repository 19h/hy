//! Context observations and virtualenv version lookup are separate from findings.

use std::path::{Path, PathBuf};

use super::{
    paths,
    state::{State, System},
};
use crate::error::Result;
use crate::ida::python::{checks, find_command};
use crate::util::{python_integer, realpath};

fn interpreter(root: &Path) -> Option<PathBuf> {
    let candidates = if cfg!(windows) {
        [root.join("Scripts/python.exe"), root.join("python.exe")]
    } else {
        [root.join("bin/python3"), root.join("bin/python")]
    };
    candidates.into_iter().find(|path| path.is_file())
}

pub(in crate::ida::python) async fn version(root: &Path) -> Result<Option<String>> {
    if let Some(executable) = interpreter(root)
        && let Some(version) = checks::version(&executable).await?
    {
        return Ok(Some(version));
    }
    Ok(configured_version(root))
}

fn configured_version(root: &Path) -> Option<String> {
    let config = paths::config(root);
    let raw = config
        .get("version")
        .filter(|value| !value.is_empty())
        .or_else(|| config.get("version_info"))?;
    let mut parts = raw.split('.');
    Some(format!(
        "{}.{}",
        python_integer::parse(parts.next()?)?,
        python_integer::parse(parts.next()?)?
    ))
}

pub(super) async fn collect(state: &State) -> Result<Vec<String>> {
    let mut notes = Vec::new();
    let recommended = state.recommended_venv();
    if state.idausr.is_some() && recommended.join("pyvenv.cfg").is_file() {
        let in_use = state
            .venv_root
            .as_deref()
            .and_then(|root| realpath::resolve(root).ok())
            .is_some_and(|root| Some(root) == realpath::resolve(&recommended).ok());
        if !in_use {
            let version = version(&recommended)
                .await?
                .map(|version| format!(" (Python {version})"))
                .unwrap_or_default();
            let executable =
                interpreter(&recommended).unwrap_or_else(|| state.system.python(&recommended));
            notes.push(format!(
                "A virtual environment already exists at {}{version}, but IDA is not configured to use it. \
                 If its version is correct, set: {}", recommended.display(), state.system.export(&executable)
            ));
        }
    }
    notes.push(if find_command("uv").is_some() {
        "uv is on PATH. `create-environment` uses `uv venv --seed`, which can also download Python."
    } else {
        "uv is not on PATH. `create-environment` uses the stdlib venv module and needs a matching Python \
         installation. To install uv, see https://docs.astral.sh/uv/."
    }.into());
    match state.system {
        System::Mac => notes.push(
            "On macOS, a shell profile applies only to IDA started from that shell, not from Finder or the Dock. \
             Use `launchctl setenv IDAPYTHON_VENV_EXECUTABLE <path>`, or start IDA from a terminal.".into()
        ),
        System::Windows => notes.push(
            "On Windows, `setx` sets the variable for your user account. Programs started afterwards, including \
             IDA from the Start menu, see it.".into()
        ),
        System::Linux => (),
    }
    Ok(notes)
}
