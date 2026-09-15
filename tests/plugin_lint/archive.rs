//! Compare archive reports and terminal failures with the pinned source command.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};
#[cfg(unix)]
use sha2::{Digest, Sha256};

use super::support::{Sandbox, assert_success};

#[path = "archive/cases.rs"]
mod cases;

/// Preserve report order and locations while excluding model-specific diagnostics.
fn report_events(report: &str) -> Vec<String> {
    report
        .lines()
        .filter(|line| {
            !line.trim_start().starts_with("Invalid value")
                && !line.trim_start().starts_with("Missing required field")
        })
        .map(|line| {
            if let Some((location, _)) = line.split_once("ida-plugin.json validation failed") {
                format!("{location}ida-plugin.json validation failed")
            } else {
                line.to_owned()
            }
        })
        .collect()
}

#[test]
fn archive_reports_match_source_order_named_reads_and_lexical_parents() {
    let sandbox = Sandbox::new();
    let root = sandbox.path().canonicalize().unwrap();
    let mut paths = Vec::new();
    let mut expected = Vec::new();
    for (index, bytes) in cases::archives().iter().enumerate() {
        let path = root.join(format!("{index}.zip"));
        fs::write(&path, bytes).unwrap();
        let output = sandbox.run(&["plugin", "lint", path.to_str().unwrap()]);
        let report = String::from_utf8(output.stdout).unwrap();
        expected.push(json!({
            "success": output.status.success(),
            "events": report_events(&report.replace(root.to_str().unwrap(), "<ROOT>")),
        }));
        paths.push(path);
    }
    if let Some(python) = std::env::var_os("HY_TEST_LINT_ORACLE_PYTHON") {
        let source =
            std::env::var_os("HY_TEST_HCLI_SOURCE").map(std::path::PathBuf::from).unwrap_or_else(
                || Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli"),
            );
        let mut child = Command::new(python)
            .args(["-I", "-B", "-c", include_str!("archive/reference.py")])
            .arg(source)
            .env("HOME", sandbox.path())
            .env("USERPROFILE", sandbox.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(&serde_json::to_vec(&paths).unwrap()).unwrap();
        let output = child.wait_with_output().unwrap();
        assert_success(&output);
        let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(actual.len(), expected.len());
        for (index, actual) in actual.iter().enumerate() {
            let report = actual["output"].as_str().unwrap();
            let actual = json!({
                "success": actual["success"],
                "events": report_events(&report.replace(root.to_str().unwrap(), "<ROOT>")),
            });
            assert_eq!(actual, expected[index], "{}", paths[index].display());
        }
    }
    assert_eq!(expected.len(), 181);
    #[cfg(unix)]
    assert_eq!(
        format!("{:x}", Sha256::digest(serde_json::to_vec(&expected).unwrap())),
        "bbd77afa9685de30a2fc6006c58d321cae788445372f20e29aab10b179bf4bc8",
    );
}

#[test]
fn reference_errors_follow_descriptor_errors_without_hiding_valid_descriptors() {
    let sandbox = Sandbox::new();
    let path = sandbox.path().join("plugin.zip");
    fs::write(&path, cases::missing_reference_then_invalid_descriptor()).unwrap();
    let output = sandbox.run(&["plugin", "lint", path.to_str().unwrap()]);
    assert_success(&output);
    let report = String::from_utf8(output.stdout).unwrap();
    let invalid = report.find("b/ida-plugin.json: ida-plugin.json validation failed").unwrap();
    let missing = report.find("a/ida-plugin.json: ida-plugin.json validation failed").unwrap();
    assert!(invalid < missing, "{report}");
    assert!(!report.contains("No valid plugins"), "{report}");
    assert!(!report.contains("Recommendation"), "{report}");
}
