use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

use super::Repository;

mod cases;

#[test]
fn graphql_and_cached_models_match_upstream_validation_and_serialization() {
    let cases = cases::all();
    assert_eq!(cases.len(), 910);
    let expected: Vec<_> = cases
        .iter()
        .map(|case| {
            let result = if case["kind"] == "graphql" {
                Repository::from_graphql(&case["value"])
            } else {
                serde_json::from_value::<Repository>(case["value"].clone()).map_err(Into::into)
            };
            match result {
                Ok(repository) => json!({"value": repository}),
                Err(_) => json!({"error": true}),
            }
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
    let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
        assert_eq!(actual, expected, "case {index}: {}", cases[index]);
    }
    eprintln!("matched {} upstream GitHub model cases", cases.len());
}

#[test]
fn releases_validate_required_fields_and_unwrap_only_one_tag_level() {
    let baseline = cases::graphql();
    assert!(Repository::from_graphql(&baseline).is_ok());
    for field in ["createdAt", "publishedAt", "isPrerelease", "isDraft", "url", "tag"] {
        let mut value = baseline.clone();
        value["releases"]["nodes"][0].as_object_mut().unwrap().remove(field);
        assert!(Repository::from_graphql(&value).is_err(), "{field}");
    }
    let mut value = baseline;
    let target = &mut value["releases"]["nodes"][0]["tag"]["target"];
    *target = json!({"target": {"target": target.clone()}});
    assert!(Repository::from_graphql(&value).is_err());
}
