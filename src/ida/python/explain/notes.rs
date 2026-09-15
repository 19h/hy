//! Ordered context notes; native Rust has no HCLI-owned Python virtualenv.

use super::types::*;
use crate::util::python_integer;

pub(super) fn collect(
    environment: &PythonEnvironment,
    venv: Option<&VirtualEnvironment>,
    version: &PythonVersion,
    binary: &str,
) -> Vec<Note> {
    let mut notes = Vec::new();
    let process = environment.virtual_env.as_deref().filter(|value| !value.is_empty());
    let user = environment.user_virtual_env.as_deref();
    let uv = environment.virtual_env_is_uv_cache;
    let mut add = |kind, text| {
        notes.push(Note {
            kind,
            text,
        })
    };
    if uv {
        add("diagnostic", match user {
            Some(user) => format!("$VIRTUAL_ENV is a uv cache overlay. Resolved user virtualenv: {}", user.display()),
            None => "$VIRTUAL_ENV is a uv cache overlay, not your virtualenv. No user virtualenvs were found on $PATH.".into(),
        });
    } else if let Some(process) = process
        && venv.is_none()
    {
        add(
            "diagnostic",
            format!(
                "$VIRTUAL_ENV is set ({process}) but was not detected inside IDA. To use this virtualenv \
                    with IDA, set $IDAPYTHON_VENV_EXECUTABLE to its interpreter."
            ),
        );
    }
    if venv.is_none() {
        add(
            "hint",
            format!(
                "To use a virtualenv with IDA, run `{binary} ida python create-environment`, or check the \
                    current setup with `{binary} ida python doctor`."
            ),
        );
    }
    if user.is_none() && !uv && venv.is_none() {
        add(
            "hint",
            "To change IDA's Python, use idapyswitch to point at a different interpreter.".into(),
        );
    }
    if environment.externally_managed {
        let executable = environment
            .python_exe
            .as_deref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "None".into());
        add(
            "warning",
            format!(
                "{executable} is an externally-managed Python (PEP 668); pip will refuse to install plugin \
                    dependencies into it directly. Point IDA at a virtual environment instead: `{binary} ida \
                    python create-environment`."
            ),
        );
    }
    if let Some(version) = version.probed_version.as_deref()
        && is_old_version(version)
    {
        add(
            "warning",
            format!(
                "Python {version} has reached end-of-life. Many IDA plugins may not support it. Consider \
                    upgrading to a newer Python and using idapyswitch to point IDA at it."
            ),
        );
    }
    notes
}

fn is_old_version(version: &str) -> bool {
    let mut parts = version.split('.');
    let parse = |part: Option<&str>| python_integer::parse(part?);
    let (Some(major), Some(minor)) = (parse(parts.next()), parse(parts.next())) else {
        return false;
    };
    (major, minor) <= (3.into(), 9.into())
}
