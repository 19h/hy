use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::*;

mod cases;

#[test]
fn aliased_queries_and_batch_envelopes_match_the_upstream_graphql_client() {
    let cases = cases::all();
    assert_eq!(cases.len(), 117);
    let expected: Vec<_> = cases.iter().map(observe).collect();
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
    eprintln!("matched {} upstream GraphQL cases", cases.len());
}

fn observe(case: &Value) -> Value {
    let names: Vec<String> = serde_json::from_value(case["repositories"].clone()).unwrap();
    if names.is_empty() {
        return json!({"request": null, "outcome": {"repositories": []}});
    }
    let mut request = request(&names).unwrap();
    let query = request["query"].as_str().unwrap();
    let normalized = query.split_whitespace().collect::<Vec<_>>().join(" ");
    request["query"] = json!(normalized);
    let outcome = match decode(&names, case["response"].clone()) {
        Ok(repositories) => {
            let names: Vec<_> = repositories.into_iter().map(|(name, _)| name).collect();
            json!({"repositories": names})
        }
        Err(Error::Other(message)) if message.starts_with("GraphQL errors:") => {
            json!({"fatal": message})
        }
        Err(_) => json!({"error": true}),
    };
    json!({"request": request, "outcome": outcome})
}

#[test]
fn fatal_graphql_errors_precede_invalid_data_and_keep_all_non_not_found_records() {
    let errors = json!([
        {"type":"NOT_FOUND", "message":"missing"},
        {"type":"FORBIDDEN", "message":"denied", "path":["repo0"]},
        {"message":"untyped", "retry":false},
    ]);
    let error = decode(&["owner/repo".into()], json!({"data":null, "errors":errors})).unwrap_err();
    assert_eq!(
        error.to_string(),
        "GraphQL errors: [{'type': 'FORBIDDEN', 'message': 'denied', 'path': ['repo0']}, {'message': 'untyped', 'retry': False}]"
    );
}
