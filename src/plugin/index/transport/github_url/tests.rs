use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

use super::*;

#[test]
fn recognized_urls_preserve_source_owner_repository_and_raw_tag() {
    let mut values = Vec::new();
    for scheme in ["https", "HTTPS", "httpſ", "http"] {
        for host in ["github.com", "GitHub.Com", "gıthub.com", "github.com:443"] {
            for owner in ["owner", ".", "..", "İſKı"] {
                for repo in
                    ["repo", ".", "..", ".git", "repo.git", "repo.git.git", "repo.GIT", "İſKı"]
                {
                    for tag in ["", "@v1", "@v1/part+build", "@/", "@//", "@İſKı", "@"] {
                        for suffix in ["", "/", "\n", "/\n", "\r\n", "?q", "#fragment"] {
                            values.push(format!("{scheme}://{host}/{owner}/{repo}{tag}{suffix}"));
                        }
                    }
                }
            }
        }
    }
    assert_eq!(values.len(), 25_088);
    let expected: Vec<_> = values
        .iter()
        .map(|value| {
            let parsed =
                parse(value).ok().map(|source| (source.owner, source.repository, source.tag));
            json!({"recognized": is_direct_github(value), "parsed": parsed})
        })
        .collect();
    assert_eq!(parse("https://github.com/o/repo.git/").unwrap().repository, "repo.git");
    assert_eq!(parse("https://github.com/o/repo.git.git").unwrap().repository, "repo.git");
    assert_eq!(parse("https://github.com/o/..@v1/\n").unwrap().tag.as_deref(), Some("v1/\n"));
    let Some(python) = std::env::var_os("HY_TEST_BUNDLE_ORACLE_PYTHON") else {
        return;
    };
    let source = std::env::var_os("HY_TEST_HCLI_SOURCE").map(PathBuf::from).unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli")
    });
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", include_str!("reference.py")])
        .arg(source)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&values).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "source oracle failed");
    let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), expected.len());
    for ((actual, expected), value) in actual.iter().zip(expected).zip(values) {
        assert_eq!(*actual, expected, "{value:?}");
    }
}
