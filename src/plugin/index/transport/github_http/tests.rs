use std::cell::RefCell;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Serialize;
use serde_json::{Value, json};

use super::*;

mod cases;

#[derive(Clone, Serialize)]
struct Reply {
    status: u16,
    headers: Vec<(String, Vec<u8>)>,
    body: Vec<u8>,
}

impl Reply {
    fn new(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: br#"{"assets":[{"name":"a.zip","browser_download_url":"https://mirror.test/asset"}]}"#.to_vec(),
        }
    }

    fn header(mut self, name: &str, value: &[u8]) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    fn response(&self) -> reqwest::Response {
        let mut response = hyper::Response::builder().status(self.status);
        for (name, value) in &self.headers {
            response = response.header(name, value.as_slice());
        }
        response.body(self.body.clone()).unwrap().into()
    }
}

#[derive(Serialize)]
struct Case {
    base: String,
    replies: Vec<Reply>,
}

fn report(result: Result<Vec<u8>>) -> Value {
    match result {
        Ok(_) => json!({"kind": "success"}),
        Err(Error::RepositoryHttp {
            status,
            url,
        }) => json!({"kind": "http", "status": status, "url": url}),
        Err(Error::Other(message)) => {
            let kind = if message.starts_with("HTTPS request was redirected") {
                "downgrade"
            } else if message.starts_with("Exceeded maximum") {
                "limit"
            } else if message.starts_with("response decoding failed") {
                "decode"
            } else {
                panic!("unexpected error: {message}")
            };
            json!({"kind": kind})
        }
        Err(error) => panic!("unexpected error: {error}"),
    }
}

#[tokio::test]
async fn redirect_and_final_response_policy_matches_upstream_github_fetch() {
    let cases = cases::all();
    assert_eq!(cases.len(), 133);
    let mut expected = Vec::new();
    for case in &cases {
        let requests = RefCell::new(Vec::new());
        let url = format!("{}/repos/o/r/releases/latest", case.base);
        let result = fetch_using(&url, "application/vnd.github.v3+json", |url, headers| {
            let names = ["accept", "accept-encoding", "authorization", "cookie"];
            let observed: serde_json::Map<_, _> = names
                .into_iter()
                .map(|name| {
                    (name.into(), json!(headers.get(name).map(|value| value.to_str().unwrap())))
                })
                .collect();
            let reply = case.replies[requests.borrow().len()].response();
            requests.borrow_mut().push(json!({"url": url.as_str(), "headers": observed}));
            std::future::ready(Ok(reply))
        })
        .await;
        expected.push(json!({"requests": requests.into_inner(), "result": report(result)}));
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
        assert_eq!(actual, expected, "case {index}");
    }
}
