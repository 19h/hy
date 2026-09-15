//! Ordered diagnostic policy from the pinned upstream environment checker.

use serde::Serialize;

use super::state::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Severity {
    Warning,
    Error,
}

#[derive(Debug, Serialize)]
pub(super) struct Finding {
    pub id: &'static str,
    pub severity: Severity,
    pub summary: String,
    pub detail: String,
    pub fix_hint: String,
}

impl Finding {
    pub fn new(
        id: &'static str,
        severity: Severity,
        summary: impl Into<String>,
        detail: impl Into<String>,
        fix_hint: impl Into<String>,
    ) -> Self {
        Self {
            id,
            severity,
            summary: summary.into(),
            detail: detail.into(),
            fix_hint: fix_hint.into(),
        }
    }
}

pub(super) fn check(state: &State, binary: &str) -> Vec<Finding> {
    use Severity::{Error, Warning};

    let mut findings = Vec::new();
    let exe = state.python_exe.display();
    if state.uv_ephemeral {
        let root = state
            .venv_root
            .as_deref()
            .map_or_else(|| "None".into(), |path| path.display().to_string());
        findings.push(Finding::new(
            "uv-ephemeral",
            Error,
            format!("IDA's Python resolved to a temporary uv environment: {root}"),
            "`uv run --with` creates this virtualenv for one command and discards it afterwards. \
             IDA never loads it. Packages installed into it are lost when the command exits.",
            format!(
                "Install HCLI permanently, for example with `uv tool install ida-hcli`. \
                 Or set $IDAPYTHON_VENV_EXECUTABLE to IDA's real virtualenv.\n{}",
                state.create_hint(binary)
            ),
        ));
    }
    let dangling =
        state.idapython_venv_executable.is_some() && !state.idapython_venv_executable_exists;
    if let Some(path) = &state.idapython_venv_executable
        && dangling
    {
        findings.push(Finding::new(
            "venv-exe-not-found", Error,
            format!("$IDAPYTHON_VENV_EXECUTABLE points to a file that does not exist: {}", path.display()),
            "The variable is set, but the interpreter it names is not on disk. HCLI fell back to \
             probing IDA directly, which found a different Python. The virtual environment may have \
             been deleted, moved, or not yet created.",
            state.create_hint(binary),
        ));
    }
    let override_active = state.source == "$HCLI_CURRENT_IDA_PYTHON_EXE";
    if override_active {
        findings.push(Finding::new(
            "hcli-override-active", Warning,
            format!("$HCLI_CURRENT_IDA_PYTHON_EXE overrides normal Python detection: {exe}"),
            "This variable makes HCLI use a specific interpreter without consulting IDA. \
             IDA does not read it, so IDA may load a different Python than the one HCLI installs into. \
             It is intended for test harnesses, not normal use.",
            "Unset HCLI_CURRENT_IDA_PYTHON_EXE and use $IDAPYTHON_VENV_EXECUTABLE instead.\n\
             That variable is read by both IDA and HCLI, so they stay in sync.",
        ));
    }
    if !state.python_exe_exists {
        findings.push(Finding::new(
            "python-exe-not-found",
            Error,
            format!("IDA's Python interpreter does not exist: {exe}"),
            format!(
                "The interpreter was selected by {}, but the file is not on disk. \
                 HCLI cannot install packages or run scripts with a missing interpreter.",
                state.source
            ),
            state.create_hint(binary),
        ));
        if override_active {
            return findings;
        }
    }
    if state.venv_root.is_none() && state.python_exe_exists {
        let mut detail = String::from(
            "IDA loads a global Python (system, Homebrew, python.org, or Windows Store). Plugin dependencies \
             installed there mix with OS packages and may need administrator rights. Debian, Ubuntu 24.04+, and \
             Homebrew refuse such installs (PEP 668). A dedicated virtual environment avoids these problems.",
        );
        if state.conda {
            detail.push_str(
                "\nThis looks like a conda environment. conda environments have no pyvenv.cfg, so HCLI cannot \
                 check or manage them like a virtualenv."
            );
        }
        findings.push(Finding::new(
            "no-venv",
            Error,
            format!("IDA's Python is not a virtual environment: {exe}"),
            detail,
            state.create_hint(binary),
        ));
    }
    if state.externally_managed {
        findings.push(Finding::new(
            "externally-managed", Error,
            format!("IDA's Python is externally managed (PEP 668): {exe}"),
            "The distributor marked this Python as EXTERNALLY-MANAGED, so pip refuses to install packages \
             into it. Do not remove the marker: OS updates can overwrite or break packages that pip \
             installs there.",
            state.create_hint(binary),
        ));
    }
    if state.pip_available == Some(false) && state.python_exe_exists {
        let hint = if let Some(root) = &state.venv_root {
            let version =
                state.python_version.as_deref().filter(|v| !v.is_empty()).unwrap_or("3.X");
            format!(
                "If you created this virtualenv with `uv venv` without `--seed`, recreate it with \
                 `uv venv --seed --python {version} {}`. \
                 Or add pip with `{exe} -m ensurepip --upgrade`.",
                root.display()
            )
        } else {
            format!(
                "Install pip with `{exe} -m ensurepip --upgrade`. Better: use a virtual environment."
            )
        };
        findings.push(Finding::new(
            "no-pip", Error, format!("pip is not available in IDA's Python: {exe}"),
            "HCLI installs plugin dependencies with pip inside IDA's Python. Without pip, HCLI cannot \
             install any plugin that has Python dependencies.", hint,
        ));
    }
    if let (Some(python), Some(ida)) = (&state.python_version, &state.ida_python_version)
        && python != ida
    {
        findings.push(Finding::new(
            "version-mismatch", Error,
            format!(
                "Python version mismatch: IDA runs Python {ida}, \
                 but HCLI would install dependencies for Python {python} ({exe})"
            ),
            format!(
                "idapyswitch selects the libpython that IDA loads, and that alone sets the Python version \
                 inside IDA. A virtualenv only changes sys.path, not the version. Packages installed for \
                 {python} go to a site-packages directory that IDA's Python {ida} never reads."
            ),
            format!(
                "Run idapyswitch to select a Python {python} installation for IDA. \
                 Or recreate the virtualenv with Python {ida}: \
                 `{binary} ida python create-environment` selects the matching version."
            ),
        ));
    }
    if let Some(root) = &state.venv_root
        && !state.variable_selects_venv
        && !dangling
    {
        let summary = state.idapython_venv_executable.as_ref().map_or_else(
            || "$IDAPYTHON_VENV_EXECUTABLE is not set".into(),
            |path| format!(
                "$IDAPYTHON_VENV_EXECUTABLE points to a different environment: {} (HCLI resolved {exe})",
                path.display()
            ),
        );
        findings.push(Finding::new(
            "no-venv-exe-var", Warning, summary,
            "IDAPYTHON_VENV_EXECUTABLE tells IDA which virtualenv to use, however IDA starts (terminal, \
             Dock, or file association). Other methods, such as idapythonrc.py or an activated shell, work \
             only in some situations. HCLI then has to guess which environment IDA uses.",
            format!(
                "Set it for your user account, then restart IDA:\n  {}",
                state.system.export(&state.system.python(root))
            ),
        ));
    }
    if state.idapythonrc_activates_venv {
        let path = state
            .idapythonrc_path
            .as_deref()
            .map_or_else(|| "None".into(), |path| path.display().to_string());
        findings.push(Finding::new(
            "idapythonrc-venv", Warning,
            format!("{path} appears to activate a virtualenv at startup"),
            "A virtualenv activated from idapythonrc.py works in interactive IDA only. idat and HCLI's \
             environment probe see the base interpreter, so HCLI can install packages where IDA never \
             looks. The script is arbitrary Python, so HCLI cannot tell which virtualenv it selects.",
            "Set $IDAPYTHON_VENV_EXECUTABLE to the virtualenv's interpreter. \
             Then remove the activation code from idapythonrc.py.",
        ));
    }
    findings
}
