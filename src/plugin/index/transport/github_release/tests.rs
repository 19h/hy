use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::json;

use super::*;

mod cases;

fn report(result: Result<String>) -> Value {
    match result {
        Ok(url) => json!({"kind": "selected", "url": url}),
        Err(Error::Other(message))
            if message.starts_with("No .zip")
                || message.starts_with("Multiple .zip")
                || message.starts_with("Asset ") =>
        {
            json!({"kind": "policy", "message": message})
        }
        Err(_) => json!({"kind": "invalid"}),
    }
}

#[test]
fn release_selection_matches_the_actual_upstream_fetch_function() {
    let documents = cases::documents();
    assert_eq!(documents.len(), 666);
    let mut cases = Vec::new();
    let mut expected = Vec::new();
    for tag in [None, Some(""), Some("v1/part+build")] {
        let source = ReleaseSource {
            owner: "Owner".into(),
            repository: "Repo.git".into(),
            tag: tag.map(str::to_owned),
        };
        for bytes in &documents {
            cases.push(json!({"bytes": bytes, "tag": tag}));
            expected.push(json!({
                "endpoint": format!("https://api.github.com{}", endpoint(&source)),
                "result": report(select(&source, bytes)),
            }));
        }
    }
    let Some(python) = std::env::var_os("HY_TEST_BUNDLE_ORACLE_PYTHON") else {
        return;
    };
    let upstream =
        std::env::var_os("HY_TEST_HCLI_SOURCE").map(PathBuf::from).unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli")
        });
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", include_str!("reference.py")])
        .arg(upstream)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&cases).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
        assert_eq!(actual, expected, "case {index}: {}", cases[index]);
    }
}

#[test]
fn irrelevant_assets_and_numeric_size_boundaries_do_not_block_selection() {
    let source = ReleaseSource {
        owner: "o".into(),
        repository: "r".into(),
        tag: None,
    };
    for size in [json!(-1), json!(true), json!(104857600), json!(0.5)] {
        let bytes = serde_json::to_vec(&json!({"assets": [
            {"name": "README.txt"}, {},
            {"name": "plugin.ZIP", "browser_download_url": "https://example.test/a", "size": size},
        ]}))
        .unwrap();
        assert_eq!(select(&source, &bytes).unwrap(), "https://example.test/a");
    }
    let error = select(
        &source,
        br#"{"assets":[{"name":"a.zip","size":104857601,"browser_download_url":"x"}]}"#,
    )
    .unwrap_err()
    .to_string();
    assert_eq!(error, "Asset a.zip (104857601 bytes) exceeds maximum size limit (104857600 bytes)");
}
