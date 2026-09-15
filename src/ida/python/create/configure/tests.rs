use serde_json::{Value, json};

use super::{Outcome, platform};
use platform::{Kind, StepResult};

#[test]
fn configuration_reports_match_upstream_including_all_skipped_and_empty_plans() {
    let scenarios = [
        (vec![], true, None),
        (vec![(Kind::ShellProfile, true, true)], true, None),
        (vec![(Kind::ShellProfile, true, false)], true, Some("shell-profile")),
        (vec![(Kind::WindowsUserEnv, false, false)], false, None),
        (
            vec![(Kind::MacosLaunchagent, true, false), (Kind::MacosLaunchctlSetenv, false, false)],
            false,
            None,
        ),
        (
            vec![(Kind::MacosLaunchctlSetenv, false, false), (Kind::ShellProfile, true, false)],
            false,
            None,
        ),
        (
            vec![(Kind::LinuxEnvironmentD, true, false), (Kind::ShellProfile, true, true)],
            true,
            Some("linux-environment-d"),
        ),
        (
            vec![(Kind::LinuxEnvironmentD, true, true), (Kind::ShellProfile, true, false)],
            true,
            Some("shell-profile"),
        ),
    ];
    let mut cases = Vec::new();
    for (steps, configured, via) in scenarios {
        let results: Vec<_> = steps
            .iter()
            .map(|&(kind, success, skipped)| StepResult {
                kind,
                success,
                skipped,
                message: "fixture".into(),
            })
            .collect();
        let result = Outcome::from_results(&results);
        assert_eq!(result.configured, configured);
        assert_eq!(result.via.as_deref(), via);
        cases.push(json!({"steps": steps, "expected": [configured, via]}));
    }
    compare_upstream(&cases);
}

fn compare_upstream(cases: &[Value]) {
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
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(cases).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual, cases.iter().map(|case| case["expected"].clone()).collect::<Vec<_>>());
}
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
