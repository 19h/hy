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
                .map(|archive| {
                    let resource = archive.cache_resource();
                    let identity: Value =
                        serde_json::from_str(resource.strip_prefix("archive-v2/").unwrap())
                            .unwrap();
                    json!({"identity":identity, "url":archive.url})
                })
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
fn cache_identity_uses_asset_coordinates_or_commit_without_size_or_url() {
    let asset = |tag: &str, name: &str, size: i64, url: &str| Archive {
        repository: "owner/repo".into(),
        url: url.into(),
        kind: Kind::Asset {
            tag: tag.into(),
            name: name.into(),
            size: size.into(),
        },
    };
    let first = asset("v1", "a.zip", 1, "https://old.test/a");
    let changed = asset("v1", "a.zip", 104_857_601, "https://new.test/a");
    assert_eq!(first.cache_resource(), changed.cache_resource());
    assert!(!first.exceeds_download_limit());
    assert!(changed.exceeds_download_limit());
    assert_ne!(first.cache_resource(), asset("v2", "a.zip", 1, &first.url).cache_resource());
    assert_ne!(
        asset("a/b", "c", 1, "").cache_resource(),
        asset("a", "b/c", 1, "").cache_resource()
    );
}
