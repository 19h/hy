//! Plain-text report sections follow the source renderer's content and order.

use std::fmt::Write;

use super::types::*;

fn kv(output: &mut String, key: &str, value: impl std::fmt::Display, via: Option<&str>) {
    write!(output, "  {key}: {value}").unwrap();
    if let Some(via) = via.filter(|value| !value.is_empty()) {
        write!(output, "  (via {via})").unwrap();
    }
    output.push('\n');
}

pub(super) fn text(report: &EnvironmentReport) -> String {
    let mut output = String::from("Known IDA installations\n");
    for installation in &report.known_installations.installations {
        writeln!(
            output,
            "  {}  (v{})",
            installation.path.display(),
            installation.version.as_deref().filter(|value| !value.is_empty()).unwrap_or("?")
        )
        .unwrap();
    }
    if let Some(error) = &report.known_installations.error {
        kv(&mut output, "scan", error, None);
    } else if report.known_installations.installations.is_empty() {
        output.push_str("  none found\n");
    }
    output.push_str("\nSelected installation\n");
    if let Some(path) = &report.selected_installation.install_dir {
        kv(
            &mut output,
            "install dir",
            path.display(),
            report.selected_installation.install_dir_source.as_deref(),
        );
    } else if let Some(error) = &report.selected_installation.install_dir_error {
        kv(&mut output, "install dir", error, None);
    }
    output.push('\n');
    let (Some(architecture), Some(environment), Some(version)) =
        (&report.architecture_and_version, &report.python_environment, &report.python_version)
    else {
        return output;
    };
    architecture_text(&mut output, architecture);
    environment_text(&mut output, environment);
    output.push_str("IDAPython virtualenv\n");
    if let Some(venv) = &report.idapython_virtualenv {
        kv(&mut output, "venv", venv.venv.display(), Some("activated by idapythonrc.py"));
        if let Some(home) = &venv.home {
            kv(&mut output, "  home", home, None);
        }
        if let Some(include) = &venv.system_site_packages {
            kv(&mut output, "  system site-packages", include, None);
        }
        kv(
            &mut output,
            "  python version",
            venv.python_version
                .as_deref()
                .filter(|value| !value.is_empty())
                .unwrap_or("could not determine"),
            None,
        );
    } else {
        output.push_str("  none detected\n");
    }
    output.push_str("\nPython version\n");
    if let Some(executable) = &version.final_python_exe {
        kv(&mut output, "final python exe", executable.display(), None);
    }
    if let Some(value) = &version.probed_version {
        let name = version
            .final_python_exe
            .as_ref()
            .and_then(|path| path.file_name())
            .unwrap_or_default()
            .to_string_lossy();
        kv(&mut output, "probed version", value, Some(&format!("running {name}")));
    } else if let Some(error) =
        version.final_python_exe_error.as_ref().or(version.probed_version_error.as_ref())
    {
        kv(&mut output, "probed version", error, None);
    }
    kv(
        &mut output,
        "HCLI interpreter",
        version.hcli_interpreter_version,
        Some(&version.hcli_interpreter_path.display().to_string()),
    );
    output.push('\n');
    for note in report.notes.iter().filter(|note| note.kind != "warning") {
        note_text(&mut output, note);
    }
    if let Some(error) = &report.python_version_mismatch_error {
        kv(&mut output, "version mismatch check", error, None);
    }
    if !report.python_version_mismatches.is_empty() {
        output.push_str(&mismatch_warning(&report.python_version_mismatches));
        output.push_str("\n\n");
    }
    for note in report.notes.iter().filter(|note| note.kind == "warning") {
        note_text(&mut output, note);
    }
    output
}

fn architecture_text(output: &mut String, report: &ArchitectureAndVersion) {
    output.push_str("Architecture and version\n");
    if let Some(path) = &report.ida_binary {
        kv(output, "ida binary", path.display(), None);
    }
    if let Some(error) = &report.ida_binary_error {
        kv(output, "ida binary", error, None);
    }
    if let Some(error) = &report.binary_arch_error {
        kv(output, "binary arch", error, None);
    } else if let Some(path) = &report.ida_binary {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        kv(
            output,
            "binary arch",
            report.binary_arch.as_deref().unwrap_or("unknown"),
            Some(&format!("{name} binary header")),
        );
    }
    if let Some(platform) = &report.platform {
        kv(output, "platform", platform, None);
    } else if let Some(error) = &report.platform_error {
        kv(output, "platform", error, None);
    }
    if let Some(version) = &report.ida_version {
        kv(output, "ida version", version, report.ida_version_source.as_deref());
    } else if let Some(error) = &report.ida_version_error {
        kv(output, "ida version", error, None);
    }
    output.push('\n');
}

fn environment_text(output: &mut String, report: &PythonEnvironment) {
    output.push_str("Python environment\n");
    let active =
        report.virtual_env.as_deref().filter(|value| !value.is_empty()).unwrap_or("not set");
    let active = if report.virtual_env_is_uv_cache && active != "not set" {
        format!("{active}  (uv cache)")
    } else {
        active.into()
    };
    kv(output, "$VIRTUAL_ENV", active, None);
    if let Some(user) = &report.user_virtual_env {
        kv(output, "user virtualenv", user.display(), Some("resolved from $PATH"));
    }
    for candidate in &report.candidate_virtual_envs {
        kv(
            output,
            "  candidate venv",
            format!("{}  (via {})", candidate.path.display(), candidate.source),
            None,
        );
    }
    let variable = report.idapython_venv_executable.as_deref().filter(|value| !value.is_empty());
    let variable = match variable {
        Some(path) if report.idapython_venv_executable_exists == Some(true) => path.into(),
        Some(path) => format!("{path}  (not found)"),
        None => "not set".into(),
    };
    kv(output, "$IDAPYTHON_VENV_EXECUTABLE", variable, None);
    if let Some(error) = &report.idat_probe_error {
        kv(output, "idat probe", error, None);
    }
    if let Some(probe) = &report.idat_probe {
        kv(output, "idat probe", "success", None);
        for (key, value) in [
            ("  sys.prefix", Some(probe.prefix.as_str())),
            ("  sys.base_prefix", Some(probe.base_prefix.as_str())),
            ("  sys.executable", probe.executable.as_deref()),
            ("  $VIRTUAL_ENV", probe.virtual_env.as_deref()),
            ("  $IDAPYTHON_VENV_EXECUTABLE", probe.idapython_venv_executable.as_deref()),
        ] {
            kv(output, key, value.unwrap_or("None"), None);
        }
        kv(
            output,
            "  sys.version_info",
            format!("{}.{}", probe.version_major, probe.version_minor),
            None,
        );
    }
    if let Some(exe) = &report.python_exe {
        kv(output, "python exe", exe.display(), report.python_exe_source.as_deref());
    } else if let Some(error) = &report.python_exe_error {
        kv(output, "python exe", error, None);
    }
    if report.externally_managed {
        kv(output, "externally managed", "yes (PEP 668)", None);
    }
    output.push('\n');
}

fn note_text(output: &mut String, note: &Note) {
    match note.kind {
        "warning" => writeln!(output, "Warning: {}\n", note.text).unwrap(),
        "diagnostic" => writeln!(output, "Note: {}\n", note.text).unwrap(),
        _ => writeln!(output, "{}", note.text).unwrap(),
    }
}

pub(super) fn mismatch_warning(mismatches: &[VersionMismatch]) -> String {
    let Some(first) = mismatches.first() else {
        return String::new();
    };
    let mut output = format!(
        "Warning: IDA's Python version does not match the active virtualenv.\n  IDA's embedded \
            Python is {}, which is what idapyswitch registered.\n",
        first.ida_version
    );
    for mismatch in mismatches {
        writeln!(
            output,
            "  - {} is Python {}: {}",
            mismatch.other_source,
            mismatch.other_version,
            mismatch.other_path.display()
        )
        .unwrap();
    }
    output.push_str(
        "This is almost certainly a misconfiguration: activating a virtualenv does not change the \
        Python version IDA runs, so packages installed there won't be importable inside IDA.\n",
    );
    if mismatches.iter().all(|entry| entry.other_version == first.other_version) {
        write!(output, "To fix it, either run idapyswitch to point IDA at Python {}, or recreate the virtualenv with Python {}.", first.other_version, first.ida_version).unwrap();
    } else {
        output.push_str("To fix it, make these versions agree: run idapyswitch to point IDA at the virtualenv's \
            Python, or recreate the virtualenv with IDA's Python.");
    }
    output
}
