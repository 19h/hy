//! Observe clock laziness, failure precedence and subsecond wait rounding.

use std::cell::Cell;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, UNIX_EPOCH};

use reqwest::header::HeaderMap;
use serde_json::{Value, json};

use super::delay;
use crate::error::Error;
use crate::util::python_time;

fn observe(reactive: bool, headers: &HeaderMap, nanoseconds: u64, fail: bool) -> Value {
    let reads = Cell::new(0);
    let now = || {
        reads.set(reads.get() + 1);
        if fail {
            Err(Error::Other("clock".into()))
        } else {
            python_time::clock(UNIX_EPOCH + Duration::from_nanos(nanoseconds))
        }
    };
    let result = if reactive {
        delay::reactive(headers, 1, now).map(Some)
    } else {
        delay::proactive(headers, now)
    };
    let outcome = match result {
        Ok(wait) => json!({"wait_ns": wait.map(|wait| u64::try_from(wait.as_nanos()).unwrap())}),
        Err(Error::GitHubValue(_)) => json!({"error": "header"}),
        Err(Error::Other(message)) => {
            json!({"error": if message == "clock" { "clock" } else { "overflow" }})
        }
        Err(error) => panic!("unexpected delay error: {error}"),
    };
    json!({"clock_reads": reads.get(), "outcome": outcome})
}

#[test]
fn rate_limit_clock_order_and_precision_match_source() {
    let mut cases = Vec::new();
    let mut expected = Vec::new();
    let huge = "9".repeat(309);
    for reactive in [false, true] {
        for retry_after in ["", "60", "bad"] {
            for remaining in ["", "0", "3", "bad"] {
                for reset in ["", "bad", "1700086461", "1700086430", "1700089999", &huge] {
                    for fail in [false, true] {
                        for fraction in [120, 121, 999_999_999] {
                            let nanoseconds = 1_700_086_400_000_000_000 + fraction;
                            let pairs = [
                                ("retry-after", retry_after),
                                ("x-ratelimit-remaining", remaining),
                                ("x-ratelimit-reset", reset),
                            ];
                            let mut headers = HeaderMap::new();
                            for (name, value) in pairs {
                                headers.insert(name, value.parse().unwrap());
                            }
                            cases.push(json!({
                                "reactive": reactive, "headers": pairs,
                                "now_ns": nanoseconds, "fail_clock": fail,
                            }));
                            expected.push(observe(reactive, &headers, nanoseconds, fail));
                        }
                    }
                }
            }
        }
    }
    assert_eq!(cases.len(), 864);
    let mut headers = HeaderMap::new();
    headers.insert("x-ratelimit-reset", huge.parse().unwrap());
    assert_eq!(
        observe(true, &headers, 0, true),
        json!({"clock_reads": 1, "outcome": {"error": "clock"}})
    );
    let Some(python) = std::env::var_os("HY_TEST_BUNDLE_ORACLE_PYTHON") else {
        return;
    };
    let source = std::env::var_os("HY_TEST_HCLI_SOURCE").map(PathBuf::from).unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli")
    });
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", include_str!("clock_reference.py")])
        .arg(source)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&cases).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "source clock-order oracle failed");
    let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
        assert_eq!(actual, expected, "case {index}: {}", cases[index]);
    }
    eprintln!("matched {} upstream rate-limit clock cases", cases.len());
}
