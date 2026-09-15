use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::prepare;

#[test]
fn bundle_reference_preprocessing_matches_upstream() {
    let mut cases = Vec::new();
    for prefix in ["", "repo/", "REPO/", "repo_/", "repo//"] {
        for name in ["", "foo", "Foo Bar", "a@b", "日本語", "a/b"] {
            for version in ["", "==", "==1", ">1", ">=1", ">=1,==2", "===1", "==1@invalid"] {
                for host in [
                    "",
                    "@invalid",
                    "@https://github.com/a/b",
                    "@https://plugins.hex-rays.com/a/b",
                    "@httpſ://github.com/a/b",
                ] {
                    for suffix in ["", "\n"] {
                        let input = format!("{prefix}{name}{version}{host}{suffix}");
                        let expected = match prepare(&input) {
                            Ok(reference) => json!({
                                "spec": format!("{}{}", reference.name, reference.spec),
                                "host": reference.host,
                            }),
                            Err(error) => json!({"error": error.to_string()}),
                        };
                        cases.push(json!({"input": input, "expected": expected}));
                    }
                }
            }
        }
    }
    assert_eq!(cases.len(), 2400);
    compare(&cases);
    let expected: Vec<_> = cases.iter().map(|case| &case["expected"]).collect();
    assert_eq!(
        format!("{:x}", Sha256::digest(serde_json::to_vec(&expected).unwrap())),
        "9473303e1ba2905338bde865347f4146f38b8ed4464b2c306ff2d88e93170adf",
    );
}

fn compare(cases: &[Value]) {
    let Some(python) = std::env::var_os("HY_TEST_BUNDLE_ORACLE_PYTHON") else {
        return;
    };
    let source =
        std::env::var_os("HY_TEST_HCLI_SOURCE").map(std::path::PathBuf::from).unwrap_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli"),
        );
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", include_str!("reference.py")])
        .arg(source)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(cases).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), cases.len());
    for (actual, case) in actual.iter().zip(cases) {
        assert_eq!(actual, &case["expected"], "{}", case["input"]);
    }
}
