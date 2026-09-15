//! Installation rejects error findings; execution emits the same findings and continues.

use std::fmt::Write;

use super::{
    collect::{self, ProbeMode},
    findings::{self, Finding, Severity},
};
use crate::error::{Error, Result};
use crate::ida::python::{ResolvedPython, checks};

pub(super) fn warning_text(findings: &[Finding], binary: &str) -> String {
    if findings.is_empty() {
        return String::new();
    }
    let mut text = String::from(if has_errors(findings) {
        "Error: HCLI cannot install plugin dependencies into IDA's Python environment."
    } else {
        "Warning: IDA's Python environment is not the recommended setup."
    });
    for finding in findings {
        let tag = match finding.severity {
            Severity::Error => "error  ",
            Severity::Warning => "warning",
        };
        write!(text, "\n  {tag} {}", finding.summary).unwrap();
    }
    write!(
        text,
        "\nRun `{binary} ida python doctor` for details and fixes.\n\
         To skip this check: `{binary} plugin --no-python-environment-check install <name>`"
    )
    .unwrap();
    text
}

pub(super) fn error_text(findings: &[Finding], binary: &str) -> String {
    let mut text =
        String::from("HCLI cannot install plugin dependencies into IDA's Python environment:\n");
    for finding in findings {
        let severity = match finding.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        writeln!(text, "- [{severity}] {}", finding.summary).unwrap();
    }
    write!(
        text,
        "Run '{binary} ida python doctor' for details and fixes.\n\
         To skip this check: {binary} plugin --no-python-environment-check install <name>"
    )
    .unwrap();
    text
}

fn has_errors(findings: &[Finding]) -> bool {
    findings.iter().any(|finding| finding.severity == Severity::Error)
}

fn emit(findings: &[Finding], binary: &str) {
    let message = warning_text(findings, binary);
    if !message.is_empty() {
        eprintln!("{message}");
    }
}

pub(crate) async fn warn(resolved: &ResolvedPython) {
    let Ok(state) = collect::collect(resolved, ProbeMode::ResolutionOnly).await else {
        return;
    };
    let binary = &crate::config::Env::global().binary_name;
    emit(&findings::check(&state, binary), binary);
}

pub(crate) async fn validate_dependency_environment(
    resolved: &ResolvedPython,
    dependencies: &[String],
    skip_environment_check: bool,
) -> Result<()> {
    let binary = &crate::config::Env::global().binary_name;
    if !skip_environment_check
        && let Ok(state) = collect::collect(resolved, ProbeMode::AllowAdditional).await
    {
        let findings = findings::check(&state, binary);
        emit(&findings, binary);
        if has_errors(&findings) {
            return Err(Error::PluginInstall(format!(
                "Cannot install required Python dependencies: {}. Reason: {}",
                dependencies.join(", "),
                error_text(&findings, binary)
            )));
        }
    }
    // The skip flag bypasses diagnostics, not the independent pip availability requirement.
    if !checks::has_pip(&resolved.exe).await {
        let exe = resolved.exe.display();
        return Err(Error::PluginInstall(format!(
            "pip is not available in IDA's Python environment at {exe}. \
             If you created the venv with `uv venv` without `--seed`, recreate it with `uv venv --seed`. \
             Or add pip with `{exe} -m ensurepip --upgrade`. \
             Run `{binary} ida python doctor` for details."
        )));
    }
    Ok(())
}
