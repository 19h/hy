use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::Value;

pub(super) fn compare(cases: &[Value]) {
    compare_script(cases, include_str!("reference.py"));
}

pub(super) fn compare_paths(cases: &[Value]) {
    compare_script(cases, include_str!("paths.py"));
}

pub(super) fn compare_guard(cases: &[Value]) {
    compare_script(cases, include_str!("guard.py"));
}

fn compare_script(cases: &[Value], script: &str) {
    let Some(python) = std::env::var_os("HY_TEST_VENV_ORACLE_PYTHON") else {
        return;
    };
    let source =
        std::env::var_os("HY_TEST_HCLI_SOURCE").map(std::path::PathBuf::from).unwrap_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli"),
        );
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", script])
        .arg(source)
        .arg("hy")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(cases).unwrap()).unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(result.status.success());
    let actual: Vec<Value> = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(actual.len(), cases.len());
    for (actual, case) in actual.iter().zip(cases) {
        assert_eq!(*actual, case["expected"], "{case}");
    }
}
