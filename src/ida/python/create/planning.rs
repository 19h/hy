//! Version authority, registered interpreter discovery, and creation command selection.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::ida::python::layout::{candidates, store_shim};
use crate::ida::python::{Probe, find_command, probe_ida};

pub(super) struct Version {
    pub value: String,
    pub source: &'static str,
    pub probe: Option<Probe>,
}

pub(super) fn valid_version(value: &str) -> bool {
    let value = value.strip_suffix('\n').unwrap_or(value);
    let Some((major, minor)) = value.split_once('.') else {
        return false;
    };
    [major, minor].into_iter().all(|part| {
        !part.is_empty() && part.chars().all(crate::util::python_integer::is_decimal_digit)
    })
}

pub(super) async fn determine_version(
    explicit: Option<String>,
    installation: Option<&Path>,
) -> Result<Version> {
    if let Some(value) = &explicit
        && !valid_version(value)
    {
        return Err(Error::Other(format!("expected a Python version like 3.12, got {value:?}")));
    }
    let probe = match installation {
        Some(path) => crate::ida::python::probe_installation(path).await,
        None => probe_ida().await,
    };
    choose_version(explicit, probe.ok())
}

fn choose_version(explicit: Option<String>, probe: Option<Probe>) -> Result<Version> {
    if let Some(probe) = probe {
        if let Some(explicit) = explicit
            && explicit != probe.version
        {
            tracing::warn!(
                "IDA runs Python {}, but --python-version {explicit} was requested. Using {}. Run idapyswitch first if IDA must use Python {explicit}.",
                probe.version,
                probe.version
            );
        }
        return Ok(Version {
            value: probe.version.clone(),
            source: "idat probe",
            probe: Some(probe),
        });
    }
    explicit.map(|value| Version { value, source:"--python-version", probe:None }).ok_or_else(||
        Error::Other("cannot determine which Python version IDA uses: idat did not run, and no --python-version was given. Pass --python-version X.Y with the Python version that idapyswitch registered for IDA.".into()))
}

pub(super) fn registered_python(probe: Option<&Probe>) -> Option<PathBuf> {
    let probe = probe?;
    let prefix = if probe.base_prefix.is_empty() {
        &probe.prefix
    } else {
        &probe.base_prefix
    };
    if prefix.is_empty() {
        return None;
    }
    let executable = Path::new(probe.executable.as_deref().unwrap_or(""));
    if probe.virtual_env.as_deref().is_none_or(str::is_empty)
        && executable.is_file()
        && executable
            .file_name()
            .is_some_and(|name| name.to_string_lossy().to_lowercase().contains("python"))
        && !store_shim(executable)
    {
        return Some(executable.to_owned());
    }
    candidates(Path::new(prefix), Some(&probe.version))
        .into_iter()
        .find(|path| path.is_file() && !store_shim(path))
}

pub(super) async fn path_python(version: &str) -> Result<Option<PathBuf>> {
    let mut seen = Vec::new();
    for name in [format!("python{version}"), "python3".into(), "python".into()] {
        let Some(path) = find_command(&name) else {
            continue;
        };
        if seen.contains(&path) || store_shim(&path) {
            continue;
        }
        seen.push(path.clone());
        if crate::ida::python::checks::version(&path).await?.as_deref() == Some(version) {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

pub(super) struct Plan {
    pub tool: &'static str,
    pub executable: PathBuf,
    pub arguments: Vec<std::ffi::OsString>,
}

pub(super) fn plan(
    target: &Path,
    version: &str,
    registered: Option<PathBuf>,
    uv: Option<PathBuf>,
    fallback: Option<PathBuf>,
) -> Result<Plan> {
    let base = registered.or(fallback);
    if let Some(executable) = uv {
        return Ok(Plan {
            tool: "uv",
            executable,
            arguments: vec![
                "venv".into(),
                "--seed".into(),
                "--python".into(),
                base.map(PathBuf::into_os_string).unwrap_or_else(|| version.into()),
                target.into(),
            ],
        });
    }
    let executable = base.ok_or_else(|| Error::Other(format!("no Python {version} interpreter found on PATH, and uv is not installed. Install uv or Python {version}, then try again.")))?;
    Ok(Plan {
        tool: "venv",
        executable,
        arguments: vec!["-m".into(), "venv".into(), target.into()],
    })
}

#[cfg(test)]
mod tests;
