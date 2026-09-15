//! Doctor orchestration. Observations, policy and rendering have separate modules.

use std::path::PathBuf;

use crate::error::Result;
use serde::Serialize;

use super::{env_var, resolve};
use findings::{Finding, Severity};
use pattern::SetupPattern;

mod collect;
mod findings;
mod guard;
mod notes;
mod paths;
mod pattern;
mod render;
mod state;

pub(crate) use guard::{validate_dependency_environment, warn};
pub(super) use notes::version as venv_version;
pub(super) use paths::{config, normalized, shell_venv, uv_ephemeral, venv_root};

#[cfg(test)]
mod tests;

#[derive(Debug, Serialize)]
pub struct DoctorReport {
    ida_install_dir: Option<PathBuf>,
    ida_install_dir_source: Option<String>,
    ida_install_dir_error: Option<String>,
    ida_version: Option<String>,
    ida_platform: Option<String>,
    idausr: Option<PathBuf>,
    python_exe: Option<PathBuf>,
    python_exe_source: Option<String>,
    python_exe_error: Option<String>,
    python_version: Option<String>,
    ida_python_version: Option<String>,
    venv_root: Option<PathBuf>,
    pip_available: Option<bool>,
    externally_managed: Option<bool>,
    idapython_venv_executable: Option<String>,
    hcli_current_ida_python_exe: Option<String>,
    pattern: Option<SetupPattern>,
    findings: Vec<Finding>,
    notes: Vec<String>,
    pub ok: bool,
}

pub async fn doctor() -> Result<DoctorReport> {
    let (install, source, error) = match crate::ida::resolve_install_dir() {
        Ok(installation) => (Some(installation.path), Some(installation.source), None),
        Err(error) => (None, None, Some(error.to_string())),
    };
    let mut report = DoctorReport {
        ida_install_dir_source: source,
        ida_install_dir_error: error,
        ida_version: install.as_deref().and_then(crate::ida::detect_ida_version),
        ida_platform: install.as_ref().and_then(|_| crate::ida::current_ida_platform().ok()),
        ida_install_dir: install,
        idausr: Some(crate::ida::ida_user_dir()),
        python_exe: None,
        python_exe_source: None,
        python_exe_error: None,
        python_version: None,
        ida_python_version: None,
        venv_root: None,
        pip_available: None,
        externally_managed: None,
        idapython_venv_executable: env_var("IDAPYTHON_VENV_EXECUTABLE"),
        hcli_current_ida_python_exe: env_var("HCLI_CURRENT_IDA_PYTHON_EXE"),
        pattern: None,
        findings: Vec::new(),
        notes: Vec::new(),
        ok: false,
    };
    let resolved = match resolve().await {
        Ok(resolved) => resolved,
        Err(error) => {
            report.python_exe_error = Some(error.to_string());
            report.findings = unresolved_findings(
                report.idapython_venv_executable.as_deref(),
                report.python_exe_error.as_deref().unwrap_or_default(),
            );
            return Ok(report);
        }
    };
    let state = collect::collect(&resolved, collect::ProbeMode::AllowAdditional).await?;
    report.findings = findings::check(&state, &crate::config::Env::global().binary_name);
    report.pattern = Some(pattern::identify(&state, &report.findings));
    report.notes = notes::collect(&state).await?;
    report.ok = !report.findings.iter().any(|finding| finding.severity == Severity::Error);
    report.python_exe = Some(state.python_exe);
    report.python_exe_source = Some(state.source);
    report.python_version = state.python_version;
    report.ida_python_version = state.ida_python_version;
    report.venv_root = state.venv_root;
    report.pip_available = state.pip_available;
    report.externally_managed = Some(state.externally_managed);
    Ok(report)
}

fn unresolved_findings(variable: Option<&str>, error: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    let binary = &crate::config::Env::global().binary_name;
    if let Some(path) = variable
        && !std::path::Path::new(path).is_file()
    {
        findings.push(Finding::new(
            "venv-exe-not-found",
            Severity::Error,
            format!("$IDAPYTHON_VENV_EXECUTABLE points to a file that does not exist: {path}"),
            "The variable is set, but the interpreter it names is not on disk. \
             The virtual environment may have been deleted, moved, or not yet created.",
            format!(
                "Run `{binary} ida python create-environment` to create a virtual \
             environment and configure $IDAPYTHON_VENV_EXECUTABLE."
            ),
        ));
    }
    findings.push(Finding::new(
        "python-not-found",
        Severity::Error,
        "HCLI cannot determine IDA's Python interpreter",
        error,
        format!(
            "Run `{binary} ida python create-environment` to create a virtual environment \
         and set IDAPYTHON_VENV_EXECUTABLE. Or set HCLI_CURRENT_IDA_PYTHON_EXE to the interpreter \
         that IDA uses."
        ),
    ));
    findings
}
