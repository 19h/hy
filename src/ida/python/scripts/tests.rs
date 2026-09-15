//! Differential checks against the pinned interpreter helpers.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};

use super::{RUN_ENTRY_POINT, execution, lookup};

fn compare(cases: &[Value], digest: &str) {
    use sha2::{Digest, Sha256};
    let expected: Vec<_> = cases.iter().map(|case| &case["expected"]).collect();
    assert_eq!(format!("{:x}", Sha256::digest(serde_json::to_vec(&expected).unwrap())), digest);
    let Some(python) = std::env::var_os("HY_TEST_VENV_ORACLE_PYTHON") else {
        return;
    };
    let source =
        std::env::var_os("HY_TEST_HCLI_SOURCE").map(std::path::PathBuf::from).unwrap_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli"),
        );
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", include_str!("tests/reference.py")])
        .arg(source)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let input = json!({"lookup": lookup::SOURCE, "entry_point": RUN_ENTRY_POINT, "cases": cases});
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&input).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{output:?}");
    let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), cases.len());
    for (actual, case) in actual.iter().zip(cases) {
        assert_eq!(*actual, case["expected"], "{case}");
    }
}

#[test]
fn missing_script_messages_match_source_metadata_and_directory_cases() {
    let mut cases = Vec::new();
    for distribution in [None, Some(""), Some("fixture"), Some("λ package")] {
        for version in [None, Some(""), Some("1.2")] {
            for directories in [json!([]), json!(["/first", "/second directory"])] {
                for declared in [false, true] {
                    let document = json!({
                        "name": "fixture λ", "path": null, "scripts_dirs": directories,
                        "entry_point": declared.then(|| json!({
                            "name": "fixture λ", "value": "fixture:main", "group": "console_scripts",
                            "distribution": distribution, "version": version,
                        })),
                    });
                    let info: lookup::ScriptInfo =
                        serde_json::from_value(document.clone()).unwrap();
                    cases.push(json!({"mode": "message", "document": document, "expected": info.not_found_message()}));
                }
            }
        }
    }
    assert_eq!(cases.len(), 48);
    compare(&cases, "946fa7f9a3fc01b63ae021af47249de6c9f9f07b7f7bdeaeb1eeb362c694235a");
}

#[test]
fn probe_framing_and_status_match_source() {
    let mut cases = Vec::new();
    for separator in [
        "\n", "\r", "\r\n", "\u{b}", "\u{c}", "\u{1c}", "\u{1d}", "\u{1e}", "\u{85}", "\u{2028}",
        "\u{2029}",
    ] {
        for prefix in ["__hcli__:", "noise __hcli__:", " __hcli__:"] {
            for status in [0, 1, 17] {
                for stderr in [b"".as_slice(), b" \xff fixture\r\nsecond\rline\x1c"] {
                    let stdout = format!(
                        "startup{separator}{prefix}{{\"name\":\"fixture\",\"path\":\"/found\"}}{separator}"
                    );
                    #[cfg(unix)]
                    let exit_status = {
                        use std::os::unix::process::ExitStatusExt;
                        std::process::ExitStatus::from_raw(status << 8)
                    };
                    #[cfg(windows)]
                    let exit_status = {
                        use std::os::windows::process::ExitStatusExt;
                        std::process::ExitStatus::from_raw(status as u32)
                    };
                    let result = Output {
                        status: exit_status,
                        stdout: stdout.into_bytes(),
                        stderr: stderr.into(),
                    };
                    let expected = match lookup::parse(Path::new("python"), &result) {
                        Ok(info) => json!({"path": info.path}),
                        Err(error) => json!({"error": error.to_string()}),
                    };
                    assert_eq!(
                        expected.get("path").is_some(),
                        status == 0 && prefix == "__hcli__:"
                    );
                    cases.push(json!({
                        "mode": "probe", "status": status, "stdout": result.stdout,
                        "stderr": stderr, "expected": expected,
                    }));
                }
            }
        }
    }
    assert_eq!(cases.len(), 198);
    compare(&cases, "874d2222afa6dd0bae53bef03b2fdd05523ef889c14d07fa4997f2f5a49cb3a9");
}

#[test]
fn subprocess_environment_uses_the_same_strict_venv_layout_as_diagnostics() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    std::fs::write(root.join("pyvenv.cfg"), "").unwrap();
    for relative in ["bin/python", "Scripts/Python.exe", "bin/tool", "other/python", "python"] {
        let python = root.join(relative);
        let command = execution::command(&python, Path::new("program"));
        let changes = command.as_std().get_envs().collect::<std::collections::HashMap<_, _>>();
        let key = std::ffi::OsStr::new;
        assert_eq!(changes[&key("PYTHONHOME")], None);
        assert!(!changes.contains_key(key("PYTHONUTF8")));
        assert_eq!(
            changes[&key("VIRTUAL_ENV")],
            matches!(relative, "bin/python" | "Scripts/Python.exe").then(|| root.as_os_str())
        );
        assert!(command.as_std().get_current_dir().is_none());
    }
}
