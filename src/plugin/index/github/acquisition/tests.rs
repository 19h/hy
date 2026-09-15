use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

use super::*;

mod cases;

#[test]
fn planned_archive_calls_match_upstream_collection_order_and_multiplicity() {
    let cases = cases::all();
    assert_eq!(cases.len(), 550);
    let expected: Vec<_> = cases
        .iter()
        .map(|case| {
            let repositories = case
                .as_array()
                .unwrap()
                .iter()
                .map(|entry| {
                    (
                        entry["name"].as_str().unwrap().to_owned(),
                        Repository::from_graphql(&entry["metadata"]).unwrap(),
                    )
                })
                .collect();
            Plan::from_repositories(repositories)
                .into_archives()
                .map(|archive| json!({"identity": identity(&archive), "url": archive.url}))
                .collect::<Vec<_>>()
        })
        .collect();
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
    let actual: Vec<Vec<Value>> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
        assert_eq!(actual, expected, "case {index}: {}", cases[index]);
    }
    eprintln!("matched {} upstream acquisition plans", cases.len());
}

#[test]
fn size_checks_preserve_signed_arbitrary_precision_boundaries() {
    for (size, oversized) in [
        ("-184467440737095516160", false),
        ("-1", false),
        ("0", false),
        ("104857600", false),
        ("104857601", true),
        ("184467440737095516160", true),
    ] {
        let archive = Archive {
            repository: "owner/repo".into(),
            url: "https://example.test/asset".into(),
            kind: Kind::Asset {
                tag: "v1".into(),
                name: "plugin.zip".into(),
                size: serde_json::from_str(size).unwrap(),
            },
        };
        assert_eq!(archive.exceeds_download_limit(), oversized, "{size}");
    }
}

fn identity(archive: &Archive) -> Value {
    match &archive.kind {
        Kind::Asset {
            tag,
            name,
            ..
        } => json!(["asset", archive.repository, tag, name]),
        Kind::Source {
            commit,
        } => json!(["source", archive.repository, commit]),
    }
}
