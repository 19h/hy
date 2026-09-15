use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{normalize_host, parse_reference};

#[test]
fn reference_syntax_and_error_precedence_match_upstream() {
    let mut cases = Vec::new();
    for prefix in ["", "repo/", "REPO/", "repo_/", "-x/", "repo/\n", "repo//"] {
        for name in
            ["", "foo", "Foo Bar", "foo.bar", "日本語", "../x", "a\\b", "a@b", "line\nx", " x "]
        {
            for version in
                ["", "==", "==1", ">1", ">=1", "<=1", "!=1", "~=1", "===1", "==1,<=2", "=1", "~1"]
            {
                for host in [
                    "",
                    "@invalid",
                    "@https://example.test/a/b",
                    "@https://github.com/a/b",
                    "@https://GitHub.com/A/B/",
                    "@https://github.com/a/b.git@v1",
                    "@https://plugins.hex-rays.com/a/b",
                    "@https://plugins.hex-rays.com/a/b/c/",
                    "@http://github.com/a/b",
                    "@https://github.com/a/../",
                    "@@https://github.com/a/b",
                    "@https://github.com/İ/K",
                    "@httpſ://github.com/a/b",
                    "@https://github.com/a/b?x",
                ] {
                    for suffix in ["", "\n", "\n\n"] {
                        cases.push(case(format!("{prefix}{name}{version}{host}{suffix}")));
                    }
                }
            }
        }
    }
    assert_eq!(cases.len(), 35280);
    verify(&cases, "104520347f2268e3bc42322869fddee09f167268e0b373e1ccea6bef903317b6");
}

#[test]
fn host_patterns_match_python_case_folding_and_terminal_newlines() {
    let mut cases = Vec::new();
    for scheme in ["https", "HTTPS", "http", "httpſ", " https"] {
        for host in ["github.com", "GİTHUB.COM", "plugins.hex-rays.com", "pluginſ.hex-rays.com"] {
            for path in [
                "a/b",
                "a/b/c",
                "a/b/",
                "a/b//",
                "a/b.git@v1",
                "a/b@tag/sub/",
                "a/..",
                "a/İıſK",
                "a/é",
                "a/b?x",
            ] {
                for suffix in ["", "\n", "\n\n", "\r", "\t"] {
                    let url = format!("{scheme}://{host}/{path}{suffix}");
                    cases.push(case(url.clone()));
                    cases.push(case(format!("foo==1@{url}")));
                }
            }
        }
    }
    assert_eq!(cases.len(), 2000);
    verify(&cases, "2ba861e01ede0714dc014db7addec973aec7e6fce5969b9c9f228ccb7590cf1d");
}

fn case(input: String) -> Value {
    let expected = match parse_reference(&input) {
        Ok(reference) => {
            if let Some(host) = &reference.host {
                let raw = input.rsplit_once('@').unwrap().1;
                assert_eq!(&normalize_host(raw).unwrap(), host, "{input}");
            }
            json!({
                "name": reference.name,
                "version_spec": reference.spec,
                "host": reference.host,
                "repo": reference.repo,
            })
        }
        Err(error) => json!({"error": error.to_string()}),
    };
    json!({"input": input, "expected": expected})
}

fn verify(cases: &[Value], digest: &str) {
    if let Some(python) = std::env::var_os("HY_TEST_BUNDLE_ORACLE_PYTHON") {
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
    let expected: Vec<_> = cases.iter().map(|case| &case["expected"]).collect();
    assert_eq!(format!("{:x}", Sha256::digest(serde_json::to_vec(&expected).unwrap())), digest);
}
