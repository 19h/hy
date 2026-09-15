use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{Archive, collect};

#[test]
fn local_read_order_and_deduplication_match_the_source_creation_loop() {
    let mut cases = Vec::new();
    for read_once in [false, true] {
        for platforms in [
            vec![],
            vec!["linux-x86_64"],
            vec!["linux-x86_64", "windows-x86_64"],
            vec!["linux-x86_64", "macos-arm64", "windows-x86_64"],
        ] {
            let platforms: Vec<_> = platforms.into_iter().map(str::to_owned).collect();
            for samples in [vec!["a"], vec!["a", "b"], vec!["a", "b", "a"], vec!["c", "b", "a"]] {
                for failure in [None, Some(0), Some(1), Some(2)] {
                    let mut calls = Vec::new();
                    let result = collect(&platforms, read_once, |platform| {
                        let index = calls.len();
                        calls.push(platform.map(str::to_owned));
                        if failure == Some(index) {
                            return Err(crate::error::Error::Other("fixture read failure".into()));
                        }
                        Ok(Archive::new(
                            "fixture".into(),
                            samples[index % samples.len()].as_bytes().to_vec(),
                        ))
                    });
                    let expected = match result {
                        Ok(archives) => json!({
                            "calls": calls,
                            "archives": archives.into_iter().map(archive_result).collect::<Vec<_>>(),
                        }),
                        Err(error) => json!({"calls": calls, "error": error.to_string()}),
                    };
                    cases.push(json!({
                        "read_once": read_once,
                        "platforms": platforms,
                        "samples": samples,
                        "failure": failure,
                        "expected": expected,
                    }));
                }
            }
        }
    }
    assert_eq!(cases.len(), 128);
    compare(&cases);
    let expected: Vec<_> = cases.iter().map(|case| &case["expected"]).collect();
    assert_eq!(
        format!("{:x}", Sha256::digest(serde_json::to_vec(&expected).unwrap())),
        "cdbb40e29a228f9c20e68407db52b132f2320938b7ef5657b434dac52451d64b",
    );
}

fn archive_result(archive: Archive) -> Value {
    json!({
        "name": archive.name,
        "bytes": String::from_utf8(archive.bytes).unwrap(),
        "platforms": archive.platforms,
    })
}

fn compare(cases: &[Value]) {
    let Some(python) = std::env::var_os("HY_TEST_VENV_ORACLE_PYTHON") else {
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
        assert_eq!(actual, &case["expected"], "{case}");
    }
}
