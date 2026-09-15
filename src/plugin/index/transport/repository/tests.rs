//! Exercise the production policy with the same reply sequences as upstream.

use std::cell::{Cell, RefCell};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

use super::*;

mod cases;

fn report(result: Result<Vec<u8>>, repository: Option<&str>) -> Value {
    match result {
        Ok(body) => json!({"kind": "success", "body": body}),
        Err(Error::PluginAccessDenied(mut denied)) => {
            denied.repository = repository.map(str::to_owned);
            json!({"kind": "denied", "status": denied.status, "url": denied.url,
                "authenticated": denied.authenticated, "repository": denied.repository,
                "message": denied.to_string()})
        }
        Err(Error::RepositoryHttp {
            status,
            url,
        }) => json!({"kind": "http", "status": status, "url": url}),
        Err(Error::Other(message)) if message.starts_with("too many redirects") => {
            json!({"kind": "limit"})
        }
        Err(Error::Other(message)) if message.starts_with("HTTPS request was redirected") => {
            json!({"kind": "downgrade"})
        }
        Err(error) => {
            assert!(error.to_string().contains("decod"), "unexpected failure: {error}");
            json!({"kind": "decode"})
        }
    }
}

fn compare_source(input: Value, expected: &Value) {
    let Some(python) = std::env::var_os("HY_TEST_BUNDLE_ORACLE_PYTHON") else {
        return;
    };
    let source = std::env::var_os("HY_TEST_HCLI_SOURCE").map(PathBuf::from).unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli")
    });
    let config = tempfile::tempdir().unwrap();
    let version_key = format!("{}.version", crate::config::Env::global().binary_name);
    std::fs::write(
        config.path().join("config.json"),
        serde_json::to_vec(&json!({version_key: "0.24.0"})).unwrap(),
    )
    .unwrap();
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", include_str!("reference.py")])
        .arg(source)
        .arg(config.path())
        .env("HCLI_BINARY_NAME", &crate::config::Env::global().binary_name)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&input).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "source oracle failed");
    let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    let expected = expected.as_array().unwrap();
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(actual, expected, "case {index}");
    }
}

fn request_event(url: &str, headers: &HeaderMap) -> Value {
    let names = ["accept", "accept-encoding", "connection", "x-api-key", "authorization", "cookie"];
    let observed: serde_json::Map<_, _> = names
        .into_iter()
        .map(|name| {
            let value = headers.get(name).map(|value| value.to_str().unwrap());
            (name.into(), json!(value))
        })
        .collect();
    json!({
        "url": url,
        "headers": observed,
        "user_agent": headers.contains_key(header::USER_AGENT),
    })
}

fn decode_reply(reply: &cases::Reply) -> Result<Response> {
    let mut headers = HeaderMap::new();
    for (name, value) in &reply.headers {
        headers.append(
            header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
            HeaderValue::from_bytes(value).unwrap(),
        );
    }
    let mut decoder = Decoder::new(&headers, u64::MAX);
    let mut body = decoder.decode(&reply.body)?.into_owned();
    body.extend(decoder.finish()?);
    Ok(Response {
        status: reply.status,
        headers,
        body,
    })
}

#[tokio::test]
async fn redirect_credentials_cookie_and_denial_transitions_match_source() {
    let cases = cases::all();
    assert_eq!(cases.len(), 231);
    let mut expected = Vec::new();
    for case in &cases {
        let events = RefCell::new(Vec::new());
        let count = Cell::new(0);
        let calls = Cell::new(0);
        let result = fetch_using(
            &case.url,
            |url, headers| {
                events.borrow_mut().push(request_event(&url, &headers));
                let reply = &case.replies[count.get()];
                count.set(count.get() + 1);
                std::future::ready(decode_reply(reply))
            },
            || {
                calls.set(calls.get() + 1);
                let mut headers = HeaderMap::new();
                if case.authenticated {
                    headers.insert("x-api-key", HeaderValue::from_static("fixture-key"));
                }
                std::future::ready(Ok(headers))
            },
        )
        .await;
        expected.push(json!({
            "requests": events.into_inner(),
            "auth_calls": calls.get(),
            "result": report(result, case.repository.as_deref()),
        }));
    }
    compare_source(json!({"cases": cases}), &json!(expected));
}

#[test]
fn credential_scope_uses_raw_urllib_hosts_before_http_normalization() {
    let mut urls = Vec::new();
    for scheme in ["https", "HTTPS", "http", "file", "httpſ"] {
        for host in [
            "plugins.hex-rays.com",
            "sub.plugins.hex-rays.com",
            "PLUGINS.HEX-RAYS.COM",
            "plugins.hex-rays.com.",
            "evilplugins.hex-rays.com",
            "plugins.hex-rays.com.evil",
            "%70lugins.hex-rays.com",
            "plugins.hex-rays.com:bad",
            "user@plugins.hex-rays.com",
            "[v1.sub.plugins.hex-rays.com]",
            "[::1]",
            "[::1",
            "plugins\u{ff0e}hex-rays.com",
            "plugins.hex-rays.com\u{ff1a}443",
        ] {
            for suffix in ["/a", "", "/a#fragment", "\n/a"] {
                urls.push(format!("{scheme}://{host}{suffix}"));
            }
        }
    }
    let expected: Vec<_> = urls.iter().map(|url| credential_host(url).ok()).collect();
    assert_eq!(urls.len(), 280);
    assert!(!credential_host("https://%70lugins.hex-rays.com/a").unwrap());
    compare_source(json!({"hosts": urls}), &json!(expected));
}
