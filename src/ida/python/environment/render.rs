//! Plain-text doctor rendering, kept independent of observation and policy.

use std::fmt::Write;
use std::path::Path;

use super::{DoctorReport, Finding, Severity};

fn path(value: Option<&Path>) -> Option<String> {
    value.map(|path| path.display().to_string())
}

fn kv(output: &mut String, key: &str, value: Option<&str>, source: Option<&str>) {
    let value = value.filter(|value| !value.is_empty()).unwrap_or("unknown");
    write!(output, "  {key}: {value}").unwrap();
    if let Some(source) = source.filter(|source| !source.is_empty()) {
        write!(output, " (via {source})").unwrap();
    }
    output.push('\n');
}

fn finding(output: &mut String, finding: &Finding) {
    writeln!(output, "  * {} ({})", finding.summary, finding.id).unwrap();
    for line in finding.detail.lines() {
        writeln!(output, "      {line}").unwrap();
    }
    output.push_str("      Fix:\n");
    for line in finding.fix_hint.lines() {
        writeln!(output, "      {line}").unwrap();
    }
    output.push('\n');
}

impl DoctorReport {
    pub fn print_text(&self) {
        print!("{}", self.text());
    }

    pub(super) fn text(&self) -> String {
        let mut output = String::from("IDA installation\n");
        kv(
            &mut output,
            "directory",
            path(self.ida_install_dir.as_deref()).as_deref(),
            self.ida_install_dir_source.as_deref(),
        );
        if self.ida_install_dir.is_none()
            && let Some(error) = &self.ida_install_dir_error
        {
            writeln!(output, "    {error}").unwrap();
        }
        kv(&mut output, "version", self.ida_version.as_deref(), None);
        kv(&mut output, "platform", self.ida_platform.as_deref(), None);
        kv(&mut output, "user directory ($IDAUSR)", path(self.idausr.as_deref()).as_deref(), None);
        output.push_str("\nIDA's Python\n");
        kv(
            &mut output,
            "interpreter",
            path(self.python_exe.as_deref()).as_deref(),
            self.python_exe_source.as_deref(),
        );
        if self.python_exe.is_none()
            && let Some(error) = &self.python_exe_error
        {
            writeln!(output, "    {error}").unwrap();
        }
        kv(&mut output, "interpreter version", self.python_version.as_deref(), None);
        kv(&mut output, "IDA's embedded Python", self.ida_python_version.as_deref(), None);
        let root = path(self.venv_root.as_deref());
        kv(
            &mut output,
            "virtual environment",
            root.as_deref().or(self.python_exe.as_ref().map(|_| "none")),
            None,
        );
        if let Some(pip) = self.pip_available {
            kv(
                &mut output,
                "pip",
                Some(if pip {
                    "available"
                } else {
                    "missing"
                }),
                None,
            );
        }
        if self.externally_managed == Some(true) {
            kv(&mut output, "externally managed (PEP 668)", Some("yes"), None);
        }
        kv(
            &mut output,
            "$IDAPYTHON_VENV_EXECUTABLE",
            self.idapython_venv_executable
                .as_deref()
                .or(self.python_exe.as_ref().map(|_| "not set")),
            None,
        );
        if let Some(value) = &self.hcli_current_ida_python_exe {
            kv(&mut output, "$HCLI_CURRENT_IDA_PYTHON_EXE", Some(value), None);
        }
        output.push('\n');
        if let Some(pattern) = &self.pattern {
            writeln!(output, "Setup: {}\n  {}\n", pattern.name, pattern.description).unwrap();
        }
        let binary = &crate::config::Env::global().binary_name;
        if self.findings.is_empty() {
            writeln!(
                output,
                "IDA's Python environment matches the recommended setup.\n\
                 You can install plugins with Python dependencies with `{binary} plugin install`."
            )
            .unwrap();
        } else {
            for (severity, label) in [(Severity::Error, "Errors"), (Severity::Warning, "Warnings")]
            {
                let selected: Vec<_> =
                    self.findings.iter().filter(|finding| finding.severity == severity).collect();
                if selected.is_empty() {
                    continue;
                }
                writeln!(output, "{label} ({})", selected.len()).unwrap();
                for entry in selected {
                    finding(&mut output, entry);
                }
            }
            writeln!(output,
                "To install plugins despite these findings, pass `--no-python-environment-check`:\n  \
                 {binary} plugin --no-python-environment-check install <name>\n"
            ).unwrap();
        }
        if !self.notes.is_empty() {
            output.push_str("Notes\n");
            for note in &self.notes {
                writeln!(output, "  - {note}").unwrap();
            }
        }
        output
    }
}
