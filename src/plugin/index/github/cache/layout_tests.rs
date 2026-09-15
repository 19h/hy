use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

use super::*;

#[test]
fn cache_locations_and_json_text_match_actual_source_setters() {
    let root = tempfile::tempdir().unwrap();
    let root = root.path();
    let mut cases = Vec::new();
    for component in ["v1", "", ".", "..", "a/b", "a\\b", "a\tb", "a\nb", "é", "a b", "x:y", "a\"b"]
    {
        for kind in ["metadata", "asset", "source"] {
            let parts = match kind {
                "metadata" => vec!["owner", component],
                "asset" => vec!["owner", "repo", "release-assets", component],
                _ => vec!["owner", "repo", "source-archives", component],
            };
            cases.push(json!({"kind": "path", "parts": parts, "filename": "file.zip"}));
        }
    }
    for filename in [
        "plugin.zip",
        "nested/plugin.zip",
        "../plugin.zip",
        "./plugin.zip",
        "/absolute.zip",
        "日本語.zip",
        "a\\b.zip",
    ] {
        cases.push(json!({
            "kind": "path",
            "parts": ["owner", "repo", "release-assets", "v1"],
            "filename": filename,
        }));
    }
    for names in [vec![], vec!["owner/repo"], vec!["a/é", "b/🧠", "c/\u{7f}", "d/\t\"\\"]] {
        cases.push(json!({"kind": "candidates", "value": names}));
    }
    let metadata = json!({
        "default_branch": {"commit_hash": "default", "committed_date": "2026-01-01", "zipball_url": "source"},
        "releases": [{
            "name": "Δ 🧠\n\u{7f}", "tag_name": "v1", "commit_hash": "source",
            "created_at": "2026-01-01", "published_at": "2026-01-01", "is_prerelease": false,
            "is_draft": false, "url": "release", "zipball_url": "source",
            "assets": [{"name": "plugin.zip", "content_type": "raw", "size": -1, "download_url": "asset"}],
        }],
        "tags": [],
    });
    cases.push(json!({"kind": "metadata", "value": metadata}));
    assert_eq!(cases.len(), 47);
    let expected: Vec<_> = cases.iter().map(|case| observe(root, case)).collect();
    let Some(python) = std::env::var_os("HY_TEST_BUNDLE_ORACLE_PYTHON") else {
        return;
    };
    let source = std::env::var_os("HY_TEST_HCLI_SOURCE").map(PathBuf::from).unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli")
    });
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", include_str!("layout_reference.py")])
        .arg(source)
        .arg(root)
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
    eprintln!("matched {} upstream cache layout/text cases", cases.len());
}

fn observe(root: &Path, case: &Value) -> Value {
    if case["kind"] == "path" {
        let parts: Vec<&str> =
            case["parts"].as_array().unwrap().iter().map(|part| part.as_str().unwrap()).collect();
        return match directory_path(root, &parts) {
            Ok(directory) => {
                let path = python_path::join(&directory, case["filename"].as_str().unwrap());
                json!({"directory": directory, "path": path})
            }
            Err(_) => json!({"error": true}),
        };
    }
    let name = if case["kind"] == "candidates" {
        "candidate_repos.json"
    } else {
        "owner/repo/releases.json"
    };
    let text = crate::util::json_format::sorted_ascii(&case["value"], "  ");
    json!({"path": root.join(name), "text": text})
}
