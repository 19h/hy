//! Interpreter selection from explicit overrides and IDA's observed runtime state.

use std::path::{Path, PathBuf};

use super::{Probe, ResolvedPython, env_var, environment, layout, probe_ida};
use crate::error::{Error, Result};

#[cfg(test)]
mod tests;

const PROBE_FAILED: &str = concat!(
    "failed to run idat to detect IDA's Python interpreter. ",
    "If you know the interpreter path, set HCLI_CURRENT_IDA_PYTHON_EXE=/path/to/python and try again.",
);

pub async fn resolve() -> Result<ResolvedPython> {
    if let Some(exe) = env_var("HCLI_CURRENT_IDA_PYTHON_EXE") {
        return Ok(ResolvedPython {
            exe: exe.into(),
            source: "$HCLI_CURRENT_IDA_PYTHON_EXE".into(),
            ida_probe: None,
        });
    }
    if let Some(exe) = env_var("IDAPYTHON_VENV_EXECUTABLE").filter(|p| Path::new(p).is_file()) {
        return Ok(ResolvedPython {
            exe: exe.into(),
            source: "$IDAPYTHON_VENV_EXECUTABLE".into(),
            ida_probe: None,
        });
    }
    let probe = probe_ida().await.map_err(|_| Error::PythonNotFound(PROBE_FAILED.into()))?;
    Ok(ResolvedPython {
        exe: derive(&probe, &crate::config::Env::global().binary_name)?,
        source: "derived from idat probe".into(),
        ida_probe: Some(probe),
    })
}

fn derive(info: &Probe, binary: &str) -> Result<PathBuf> {
    if info.frozen {
        return Err(Error::PythonNotFound(
            "IDA is running as a frozen application, cannot detect Python executable".into(),
        ));
    }
    let executable = info.executable.as_deref().filter(|path| !path.is_empty()).map(Path::new);
    let requested =
        info.idapython_venv_executable.as_deref().filter(|path| !path.is_empty()).map(Path::new);
    let requested_root = requested.and_then(environment::venv_root);
    let virtual_env = info
        .virtual_env
        .as_deref()
        .filter(|path| !path.is_empty())
        .and_then(|path| environment::normalized(Path::new(path)));
    let candidates = prefix_candidates(info)?;

    // A matching requested or active virtualenv takes precedence across both prefixes.
    for candidate in &candidates {
        if candidate.exists() {
            let root = environment::venv_root(candidate);
            if requested_root.is_some() && root == requested_root
                || matches_virtual_env(root.as_deref(), virtual_env.as_deref())
            {
                return Ok(candidate.clone());
            }
        }
    }
    if info.prefix != info.base_prefix
        && let Some(candidate) = candidates.iter().find(|path| path.exists())
    {
        return Ok(candidate.clone());
    }

    // Embedded macOS runtimes can report the base prefix even when a venv was requested.
    if let Some(executable) = executable.filter(|path| path.exists() && !layout::store_shim(path)) {
        let root = environment::venv_root(executable);
        let same_requested_root = requested_root.is_some() && root == requested_root;
        let same_requested_executable = requested.is_some_and(|requested| {
            environment::normalized(executable) == environment::normalized(requested)
        });
        if same_requested_root
            || matches_virtual_env(root.as_deref(), virtual_env.as_deref())
            || same_requested_executable
        {
            return Ok(executable.into());
        }
    }
    if requested_root.is_some()
        && let Some(requested) = requested.filter(|path| path.exists())
    {
        return Ok(requested.into());
    }
    if let Some(candidate) = candidates.iter().find(|path| path.exists()) {
        return Ok(candidate.clone());
    }
    Err(Error::PythonNotFound(not_found_message(info, binary, &candidates)))
}

fn prefix_candidates(info: &Probe) -> Result<Vec<PathBuf>> {
    let mut candidates = Vec::new();
    for (index, prefix) in [&info.prefix, &info.base_prefix].into_iter().enumerate() {
        if prefix.is_empty() || index == 1 && info.prefix == info.base_prefix {
            continue;
        }
        for path in layout::candidates(Path::new(prefix), Some(&info.version)) {
            candidates.push(layout::absolute(&path)?);
        }
    }
    Ok(candidates)
}

fn matches_virtual_env(root: Option<&Path>, virtual_env: Option<&Path>) -> bool {
    // Upstream normalizes str(None) when an executable has no venv root.
    virtual_env.is_some()
        && environment::normalized(root.unwrap_or(Path::new("None"))).as_deref() == virtual_env
}

fn not_found_message(info: &Probe, binary: &str, candidates: &[PathBuf]) -> String {
    let optional = |value: &Option<String>| value.as_deref().unwrap_or("None").to_owned();
    let tried = crate::util::python_repr::json_str(&serde_json::json!(candidates));
    format!(
        "Could not detect IDA's Python executable.\n\
         Run '{binary} ida python create-environment' to create a virtual environment for IDA and set IDAPYTHON_VENV_EXECUTABLE. Or run idapyswitch to select a Python installation. Then try again.\n\
         sys.prefix: {}\nsys.base_prefix: {}\nsys.executable: {}\nVIRTUAL_ENV: {}\nIDAPYTHON_VENV_EXECUTABLE: {}\nTried: {tried}",
        info.prefix,
        info.base_prefix,
        optional(&info.executable),
        optional(&info.virtual_env),
        optional(&info.idapython_venv_executable),
    )
}
