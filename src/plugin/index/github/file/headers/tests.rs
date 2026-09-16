use std::cell::{Cell, RefCell};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::{Value, json};

use super::*;
use crate::error::Error;
use crate::util::python_json::{self, Text};

fn compare_source(operation: &str, cases: &[Value], expected: &[Value]) {
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
    let input = json!({"operation": operation, "cases": cases});
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&input).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "source oracle failed");
    let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(actual, expected, "case {index}: {}", cases[index]);
    }
    eprintln!("matched {} file header {operation} cases", cases.len());
}

fn text(document: &str) -> Text {
    let python_json::Value::String(ref text) = python_json::parse(document).unwrap() else {
        panic!("fixture must contain a string")
    };
    text.clone()
}

fn timestamp_result(value: f64) -> Value {
    match timestamp::datetime(value) {
        Ok(value) => json!({"datetime": value.format("%Y-%m-%dT%H:%M:%S%.6f").to_string()}),
        Err(Error::GitHubValue(message)) => json!({"kind": "value", "message": message}),
        Err(Error::GitHubUrl(_)) => json!({"kind": "os"}),
        Err(Error::Other(message)) => json!({"kind": "overflow", "message": message}),
        Err(error) => panic!("unexpected timestamp error: {error}"),
    }
}

#[test]
fn timestamp_rounding_calendar_range_and_errors_match_source() {
    assert_eq!(timestamp_result(0.0), json!({"datetime": "1970-01-01T00:00:00.000000"}));
    assert_eq!(timestamp_result(0.0000015), json!({"datetime": "1970-01-01T00:00:00.000002"}));
    assert_eq!(
        timestamp_result(f64::NAN),
        json!({
            "kind": "value", "message": "Invalid value NaN (not a number)",
        })
    );
    assert_eq!(
        timestamp_result(f64::INFINITY),
        json!({
            "kind": "overflow", "message": "timestamp out of range for platform time_t",
        })
    );
    let mut values = Vec::new();
    for sign in [0, 1_u64 << 63] {
        for exponent in 0..=2047_u64 {
            for fraction in [0, 1, 1 << 51, (1 << 52) - 1] {
                values.push(f64::from_bits(sign | exponent << 52 | fraction));
            }
        }
    }
    for boundary in [-62_135_596_800.0_f64, 253_402_300_800.0] {
        values.push(boundary);
        let (mut lower, mut upper) = (boundary, boundary);
        for _ in 0..64 {
            lower = lower.next_down();
            upper = upper.next_up();
            values.extend([lower, upper]);
        }
    }
    for base in [-1.0, 0.0, 1.0, 100.0, 1_700_000_000.0] {
        for microseconds in [0.5, 1.5, 2.5, 499_999.5, 500_000.5, 999_998.5, 999_999.5] {
            let value: f64 = base + microseconds / 1_000_000.0;
            values.extend([value.next_down(), value, value.next_up()]);
        }
    }
    assert_eq!(values.len(), 16_747);
    let cases: Vec<_> = values.iter().map(|value| json!(value.to_bits())).collect();
    let expected: Vec<_> = values.into_iter().map(timestamp_result).collect();
    compare_source("timestamps", &cases, &expected);
}

#[test]
fn independent_mime_selector_parsing_matches_source() {
    let request = Request {
        host: "".into(),
        selector: "//[bad]/archive.zip".into(),
    };
    assert!(request.validate_mime_selector().is_err());
    let request = Request {
        host: "".into(),
        selector: text(r#""/archive-\ud800.zip""#),
    };
    assert!(request.validate_mime_selector().is_ok());
    let mut documents = Vec::new();
    for prefix in ["", "//", "http://", "file://", "\t//", "C:"] {
        for authority in [
            "",
            "localhost",
            "[bad]",
            "[::1]",
            "[::1",
            "a[b]",
            "[::1]junk",
            "[v1.future]",
            "[v1.\nfuture]",
            "[::1%scope]",
            "host\u{ff0f}tail",
            "host\u{2100}tail",
        ] {
            for suffix in ["", "/archive.zip", "?query#fragment", "/archive.zip.gz", "/a;b"] {
                documents
                    .push(serde_json::to_string(&format!("{prefix}{authority}{suffix}")).unwrap());
            }
        }
    }
    for point in 0xd800..=0xdfff {
        for template in [
            r#""/archive-\uPOINT.zip""#,
            r#""//\uPOINT/archive.zip""#,
            r#""//[::1%\uPOINT]/archive.zip""#,
            r#""//[v1.\uPOINT]/archive.zip""#,
        ] {
            documents.push(template.replace("POINT", &format!("{point:04x}")));
        }
    }
    assert_eq!(documents.len(), 8_552);
    let cases: Vec<_> = documents.iter().map(|document| json!(document)).collect();
    let expected: Vec<_> = documents
        .iter()
        .map(|document| {
            let request = Request {
                host: "".into(),
                selector: text(document),
            };
            json!(request.validate_mime_selector().is_ok())
        })
        .collect();
    compare_source("mime", &cases, &expected);
}

fn outcome(result: crate::error::Result<Vec<u8>>) -> Value {
    match result {
        Ok(bytes) => json!({"kind": "ok", "bytes": bytes}),
        Err(Error::GitHubValue(message)) => {
            let stage = if message.starts_with("year ") || message.starts_with("Invalid value NaN")
            {
                "timestamp"
            } else {
                "selector"
            };
            json!({"kind": "value", "stage": stage})
        }
        Err(Error::GitHubUrl(message)) => {
            let stage = if message.contains("file not on local host") {
                "host"
            } else {
                "timestamp"
            };
            json!({"kind": "url", "stage": stage})
        }
        Err(Error::Other(_)) => json!({"kind": "overflow", "stage": "timestamp"}),
        Err(error) => panic!("unexpected header error: {error}"),
    }
}

#[tokio::test]
async fn header_failures_precede_host_validation_and_preserve_retry_categories() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("archive.zip");
    std::fs::write(&path, b"payload").unwrap();
    let mut cases = Vec::new();
    let mut expected = Vec::new();
    for modified in [
        0.0_f64,
        -62_135_596_800.0,
        -62_135_596_801.0,
        253_402_300_799.0,
        253_402_300_800.0,
        1e18,
        1e20,
        f64::NAN,
        f64::INFINITY,
    ] {
        for selector in [
            r#""/archive.zip""#,
            r#""//[bad]/archive.zip""#,
            r#""//host\uff0ftail/archive.zip""#,
            r#""/archive-\ud800.zip""#,
        ] {
            for host in ["", "192.0.2.1"] {
                cases.push(json!({"bits": modified.to_bits(), "selector": selector, "host": host}));
                let request = Request {
                    host: host.into(),
                    selector: text(selector),
                };
                let attempts = Cell::new(0);
                let waits = RefCell::new(Vec::new());
                let result = super::super::super::retry::open_file(
                    || {
                        attempts.set(attempts.get() + 1);
                        super::super::open_after_stat(&request, &path, modified)
                    },
                    |duration| {
                        waits.borrow_mut().push(duration.as_secs_f64());
                        std::future::ready(())
                    },
                )
                .await
                .and_then(|mut file| {
                    let mut bytes = Vec::new();
                    file.read_to_end(&mut bytes)?;
                    Ok(bytes)
                });
                expected.push(json!({
                    "outcome": outcome(result),
                    "attempts": attempts.get(),
                    "waits": waits.into_inner(),
                }));
            }
        }
    }
    assert_eq!(cases.len(), 72);
    assert_eq!(expected[0]["outcome"], json!({"kind": "ok", "bytes": b"payload"}));
    assert_eq!(expected[1]["outcome"], json!({"kind": "url", "stage": "host"}));
    assert_eq!(expected[1]["waits"], json!([2.0, 4.0, 8.0]));
    assert_eq!(expected[2]["outcome"], json!({"kind": "value", "stage": "selector"}));
    assert_eq!(expected[2]["attempts"], json!(1));
    compare_source("after_stat", &cases, &expected);
}

#[test]
fn filesystem_float_timestamps_match_source_including_pre_epoch_nanoseconds() {
    let temporary = tempfile::tempdir().unwrap();
    let mut cases = Vec::new();
    let mut expected = Vec::new();
    for seconds in [-2_i64, -1, 0, 1, 1_700_000_000, 253_402_300_799] {
        for nanoseconds in [0, 1, 499, 500, 501, 999_999_499, 999_999_500, 999_999_999] {
            let path = temporary.path().join(format!("{seconds}-{nanoseconds}"));
            let file = std::fs::File::create(&path).unwrap();
            let base = if seconds < 0 {
                UNIX_EPOCH - Duration::from_secs(seconds.unsigned_abs())
            } else {
                UNIX_EPOCH + Duration::from_secs(seconds as u64)
            };
            file.set_modified(base + Duration::from_nanos(nanoseconds)).unwrap();
            // Compare the retained timestamp, including filesystem truncation/clamping.
            let modified = modified(&file.metadata().unwrap()).unwrap();
            cases.push(json!(path));
            expected.push(json!(modified.to_bits()));
        }
    }
    assert_eq!(cases.len(), 48);
    compare_source("metadata", &cases, &expected);
}
