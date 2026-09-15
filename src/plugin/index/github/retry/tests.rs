use std::cell::{Cell, RefCell};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use reqwest::header::HeaderValue;
use serde::Serialize;
use serde_json::{Value, json};

use super::*;

mod cases;

const NOW: f64 = 1_700_000_000.25;

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Step {
    Http {
        status: u16,
        headers: Vec<(String, Vec<u8>)>,
    },
    Transient,
    Timeout,
    Terminal,
}

impl Step {
    fn http(status: u16) -> Self {
        Self::Http {
            status,
            headers: Vec::new(),
        }
    }

    fn header(mut self, name: &str, value: &str) -> Self {
        let Self::Http {
            headers,
            ..
        } = &mut self
        else {
            unreachable!()
        };
        headers.push((name.into(), value.as_bytes().into()));
        self
    }

    fn response(&self) -> std::result::Result<reqwest::Response, Failure> {
        match self {
            Self::Http {
                status,
                headers,
            } => {
                let mut response = hyper::Response::builder().status(*status);
                for (name, value) in headers {
                    response = response.header(name, HeaderValue::from_bytes(value).unwrap());
                }
                Ok(response.body(Vec::<u8>::new()).unwrap().into())
            }
            Self::Transient | Self::Timeout => {
                Err(Failure::Transient(Error::Other("transport".into())))
            }
            Self::Terminal => Err(Failure::Terminal(Error::Other("terminal".into()))),
        }
    }
}

async fn observe(steps: &[Step]) -> Value {
    let clock = Cell::new(NOW);
    let events = RefCell::new(Vec::new());
    let calls = Cell::new(0);
    let result = execute(
        || {
            let index = calls.get();
            calls.set(index + 1);
            events.borrow_mut().push(json!(["request", clock.get()]));
            std::future::ready(steps[index].response())
        },
        |duration| {
            let seconds = duration.as_secs_f64();
            events.borrow_mut().push(json!(["wait", seconds]));
            clock.set(clock.get() + seconds);
            std::future::ready(())
        },
        || clock.get(),
    )
    .await;
    let outcome = match result {
        Ok(response) => json!({"status": response.status().as_u16()}),
        Err(Error::Other(message)) => json!({"error": if message.starts_with("invalid GitHub") {
            "header"
        } else if message.contains("binary64") {
            "overflow"
        } else { &message }}),
        Err(error) => panic!("unexpected retry error: {error}"),
    };
    json!({"events": events.into_inner(), "outcome": outcome})
}

#[tokio::test]
async fn nested_retry_sequences_match_the_actual_upstream_decorators() {
    let cases = cases::all();
    assert_eq!(cases.len(), 562);
    let mut expected = Vec::new();
    for case in &cases {
        expected.push(observe(case).await);
    }
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
        assert_eq!(
            actual,
            expected,
            "case {index}: {}",
            serde_json::to_string(&cases[index]).unwrap()
        );
    }
    eprintln!("matched {} upstream retry sequences", cases.len());
}

#[tokio::test]
async fn transient_retries_restart_the_inner_rate_limit_attempt_counter() {
    let steps =
        [Step::http(403), Step::http(429), Step::http(503), Step::http(403), Step::http(200)];
    let observed = observe(&steps).await;
    let waits: Vec<_> = observed["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|event| event[0] == "wait")
        .map(|event| event[1].as_f64().unwrap())
        .collect();
    assert_eq!(waits, [60.0, 120.0, 2.0, 60.0]);
    assert_eq!(observed["outcome"], json!({"status": 200}));
}

#[tokio::test]
async fn malformed_final_rate_limit_headers_fail_before_exhaustion_is_returned() {
    let mut steps = vec![Step::http(429); 4];
    steps.push(Step::http(429).header("retry-after", "invalid"));
    let observed = observe(&steps).await;
    assert_eq!(observed["outcome"], json!({"error": "header"}));
    assert_eq!(observed["events"].as_array().unwrap().len(), 9);
}

#[tokio::test(start_paused = true)]
async fn refused_connections_use_three_transient_waits_before_returning_the_error() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let client =
        reqwest::Client::builder().no_proxy().retry(reqwest::retry::never()).build().unwrap();
    let request = client.get(format!("http://{address}/fixture")).build().unwrap();
    let started = tokio::time::Instant::now();
    let error = send(&client, request).await.unwrap_err();
    assert!(matches!(error, Error::Http(error) if error.is_connect()));
    assert_eq!(started.elapsed(), Duration::from_secs(2 + 4 + 8));
}
