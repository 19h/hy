//! Setup categories in source precedence order, using already-collected observations.

use serde::Serialize;

use super::{findings::Finding, state::State};

#[derive(Debug, Serialize)]
pub(super) struct SetupPattern {
    pub id: &'static str,
    pub name: &'static str,
    pub description: String,
}

pub(super) fn identify(state: &State, findings: &[Finding]) -> SetupPattern {
    let has_venv = state.venv_root.is_some();
    let (id, name, description) = if has_venv && state.variable_selects_venv && findings.is_empty()
    {
        (
            "properly-configured",
            "Properly configured",
            "IDA loads a virtual environment selected by $IDAPYTHON_VENV_EXECUTABLE. It has pip, and its \
             Python version matches the one idapyswitch registered.",
        )
    } else if state.uv_ephemeral {
        (
            "uv-ephemeral",
            "Temporary uv environment",
            "HCLI runs under `uv run --with`. The virtualenv it sees is uv's temporary overlay, not IDA's \
             environment.",
        )
    } else if !state.python_exe_exists {
        return SetupPattern {
            id: "missing-interpreter",
            name: "Missing interpreter",
            description: format!(
                "The interpreter selected by {} does not exist on disk. \
                 HCLI cannot install packages or run scripts until this is corrected.",
                state.source
            ),
        };
    } else if state.idapython_venv_executable.is_some() && !state.idapython_venv_executable_exists {
        (
            "dangling-venv-exe",
            "Missing virtual environment",
            "$IDAPYTHON_VENV_EXECUTABLE is set, but the interpreter it names does not exist on disk. \
             The virtual environment may have been deleted, moved, or not yet created.",
        )
    } else if state.idapythonrc_activates_venv {
        (
            "idapythonrc-venv",
            "idapythonrc.py virtualenv",
            "idapythonrc.py activates a virtualenv when IDA starts. This works in interactive IDA, but idat \
             and HCLI do not see it.",
        )
    } else if has_venv
        && state.shell_virtual_env.is_some()
        && state.idapython_venv_executable.is_none()
    {
        (
            "shell-activated-venv",
            "Shell-activated virtualenv",
            "A virtualenv is active in this shell ($VIRTUAL_ENV). IDA uses it only when started from such a \
             shell. Desktop launchers and file associations start IDA with the base Python.",
        )
    } else if has_venv && state.variable_selects_venv {
        (
            "configured-with-problems",
            "Configured virtualenv with problems",
            "$IDAPYTHON_VENV_EXECUTABLE selects a virtualenv for IDA, but that environment has the problems \
             listed below.",
        )
    } else if has_venv {
        (
            "venv-not-configured",
            "Virtualenv not configured for IDA",
            "HCLI found a virtualenv, but $IDAPYTHON_VENV_EXECUTABLE does not select it. IDA may load a \
             different environment than the one HCLI installs into.",
        )
    } else if is_store_shim(&state.python_exe.to_string_lossy()) {
        (
            "windows-store",
            "Windows Store Python",
            "IDA's Python resolves to the Microsoft Store app-execution alias. This alias cannot install \
             packages, and it may not be the Python that idapyswitch registered.",
        )
    } else if state.conda {
        (
            "conda",
            "Anaconda/conda Python",
            "IDA loads a conda environment. conda environments are not standard virtualenvs, so HCLI cannot \
             check them. Installing packages mixes pip and conda packages.",
        )
    } else if state.homebrew {
        (
            "homebrew",
            "Homebrew Python",
            "IDA loads a Homebrew Python. Homebrew upgrades replace the interpreter, which breaks the \
             idapyswitch registration. Homebrew also marks it externally managed, so pip refuses to install \
             into it.",
        )
    } else {
        (
            "default",
            "Default (no setup)",
            "IDA loads a global Python. There is no virtual environment, and $IDAPYTHON_VENV_EXECUTABLE is not \
             set. This is how a fresh IDA installation looks, and it is the most common cause of plugin \
             installation problems.",
        )
    };
    SetupPattern {
        id,
        name,
        description: description.into(),
    }
}

fn is_store_shim(path: &str) -> bool {
    let lowered = path.to_lowercase();
    lowered.contains("microsoft/windowsapps") || lowered.contains("microsoft\\windowsapps")
}
