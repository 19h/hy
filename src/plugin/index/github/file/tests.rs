//! Compare catalogue-specific request parsing, paths, and actual file opens.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

use super::*;
use crate::util::python_json::{self, Text};

fn text(document: &str) -> Text {
    let python_json::Value::String(ref text) = python_json::parse(document).unwrap() else {
        panic!("fixture must contain a string")
    };
    text.clone()
}

fn compare_source(operation: &str, documents: &[String], expected: &[Value]) {
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
    let input = json!({"operation": operation, "documents": documents});
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&input).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "source oracle failed");
    let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(actual, expected, "case {index}: {}", documents[index]);
    }
    eprintln!("matched {} catalogue file {operation} cases", documents.len());
}

fn projection(url: &Text) -> Value {
    match Request::parse(url) {
        Ok(Some(request)) => {
            let selector: Vec<_> = request.selector.codepoints().collect();
            json!({
                "host": request.host.codepoints().collect::<Vec<_>>(),
                "selector": selector,
                "posix": path::posix(&selector).ok(),
                "windows": path::windows(&selector).ok(),
                "remote_double_slash": request.has_remote_double_slash(),
            })
        }
        Ok(None) => Value::Null,
        Err(_) => json!({"error": true}),
    }
}

#[test]
fn file_request_parsing_and_path_conversion_match_source() {
    let mut documents = Vec::new();
    for prefix in ["file:", "FILE:", " File:", "<URL:file:", "URL:file:"] {
        for host in [
            "",
            "//",
            "//localhost",
            "//LOCALHOST",
            "//127.0.0.1",
            "//192.0.2.1",
            "//localhost:",
            "//localhost:0",
            "//local%68ost",
            "//[::1]",
            "//[::1",
            "//host\u{ff1a}bad",
            "//%ff",
            "//[v1.example]",
            "//[v1.\nexample]",
        ] {
            for path in [
                "",
                "/",
                "//",
                "///",
                "////",
                "/a/./b//",
                "/a/../b",
                "/a?query#fragment",
                "/a%3Fb%23c",
                "/%ff/%c3%a9",
                "/%c3é",
                "/a\tb\nc\rd",
                "/%00",
                "/C:/a",
                "/C|/a",
                "/|/a",
                "/C|a|b",
                "/%2f%2fhost/a",
                "relative",
                "?query",
                "/trailing ",
                "/a\\b",
                "/;params",
                "/%ed%a0%80",
            ] {
                let closing = if prefix.starts_with('<') {
                    ">"
                } else {
                    ""
                };
                documents.push(
                    serde_json::to_string(&format!("{prefix}{host}{path}{closing}")).unwrap(),
                );
            }
        }
    }
    for point in 0xd800..=0xdfff {
        for template in [
            r#""file:/tmp/\uPOINT""#,
            r#""file://\uPOINT/tmp/a""#,
            r#""file://[::1%\uPOINT]/tmp/a""#,
            r#""file:/tmp/a#\uPOINT""#,
            r#""file:/tmp/%c3\uPOINT%a9""#,
        ] {
            documents.push(template.replace("POINT", &format!("{point:04x}")));
        }
    }
    let expected: Vec<_> = documents.iter().map(|document| projection(&text(document))).collect();
    assert_eq!(documents.len(), 12040);
    compare_source("paths", &documents, &expected);
}

#[cfg(unix)]
#[tokio::test]
async fn catalogue_file_open_and_retry_boundaries_match_source() {
    use std::cell::{Cell, RefCell};
    use std::os::unix::fs::symlink;

    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    std::fs::write(root.join("file"), b"plain").unwrap();
    std::fs::write(root.join("file?query"), b"query").unwrap();
    std::fs::write(root.join("with\ttab"), b"tab").unwrap();
    std::fs::create_dir_all(root.join("nested/inside")).unwrap();
    std::fs::write(root.join("nested/file"), b"nested").unwrap();
    symlink(root.join("nested/inside"), root.join("link")).unwrap();
    let mut documents = Vec::new();
    for authority in ["", "localhost", "127.0.0.1", "192.0.2.1", "localhost:0"] {
        for suffix in [
            "file",
            "file?query",
            "file#fragment",
            "file/",
            "file/.",
            "missing",
            "nested",
            "with\ttab",
            "link/../file",
            "%00",
            "raw-%ff",
        ] {
            documents.push(
                serde_json::to_string(&format!("file://{authority}{}/{suffix}", root.display()))
                    .unwrap(),
            );
        }
        documents.push(
            serde_json::to_string(&format!("file://{authority}/{}/file", root.display())).unwrap(),
        );
    }
    for point in ["d800", "dc00", "dc80", "dcff"] {
        documents.push(format!(r#""file://{}/\u{point}""#, root.display()));
        documents.push(format!(r#""file://\u{point}{}/file""#, root.display()));
        documents.push(format!(r#""file://\u{point}:0{}/file""#, root.display()));
    }
    let mut expected = Vec::new();
    for document in &documents {
        let request = Request::parse(&text(document)).unwrap().unwrap();
        let waits = RefCell::new(Vec::new());
        let attempts = Cell::new(0);
        let opened = super::super::retry::open_file(
            || {
                attempts.set(attempts.get() + 1);
                open(&request)
            },
            |duration| {
                waits.borrow_mut().push(duration.as_secs_f64());
                std::future::ready(())
            },
        )
        .await;
        let result = opened.and_then(|mut file| {
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            Ok(bytes)
        });
        let (kind, bytes, reason) = match result {
            Ok(bytes) => ("ok", Some(bytes), None),
            Err(Error::GitHubValue(_)) => ("value", None, None),
            Err(Error::GitHubUrl(message)) => {
                let reason = if message.contains("supported only on localhost") {
                    "remote"
                } else if message.contains("unknown url type") {
                    "unknown"
                } else if message.contains("file not on local host") {
                    "locality"
                } else {
                    "system"
                };
                ("url", None, Some(reason))
            }
            Err(_) => ("other", None, None),
        };
        expected.push(json!({
            "kind": kind,
            "bytes": bytes,
            "reason": reason,
            "attempts": attempts.get(),
            "waits": waits.into_inner(),
        }));
    }
    assert_eq!(documents.len(), 72);
    assert_eq!(expected[0]["bytes"], json!(b"plain"));
    assert_eq!(expected[1]["bytes"], json!(b"query"));
    assert_eq!(expected[3]["waits"], json!([2.0, 4.0, 8.0]));
    assert_eq!(expected[8]["bytes"], json!(b"nested"));
    compare_source("read", &documents, &expected);
}
