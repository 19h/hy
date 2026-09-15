use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use reqwest::header::{HeaderName, HeaderValue};
use serde_json::{Value, json};

use crate::error::Error;

use super::*;

const BASE: &str = "https://origin.test/a/b;old?before=1#original";

#[test]
fn redirect_targets_methods_headers_and_history_match_urllib() {
    let mut cases = Vec::new();
    for method in ["GET", "HEAD", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"] {
        for status in [200, 300, 301, 302, 303, 304, 307, 308, 400] {
            for location in ["/next", "../next", "", "http://other.test/path", "file:///tmp/file"] {
                cases.push(case(method, vec![step(status, location)]));
            }
        }
    }
    for location in [
        ".",
        "..",
        "./",
        "../",
        "../../next",
        "/../next",
        "//mirror.test",
        "///next",
        "////next",
        "a//b",
        "/a//b",
        "/a/%2e%2e/b",
        "?next=1",
        "?",
        "#next",
        "#",
        ";next",
        "a;next?query#fragment",
        "http:/next",
        "https:/next",
        "https:next",
        "https://other.test/a/../b",
        "https://other.test?query",
        "ftp://ftp.test/file",
        "data:text/plain,hello",
        "mailto:owner@example.test",
        "//[::1]/next",
        "//[broken]/",
        "//[::1",
        "//user:password@host.test:81/next",
        "//HOST.TEST/next",
        "\\next",
        " next path ",
        "next\tpath\r\n",
        "/é",
        "/\u{80}",
        "/\u{ff}",
        "/%ZZ",
        "/[x]",
    ] {
        let bytes: Vec<_> = location.chars().map(|character| character as u8).collect();
        cases.push(json!({"kind": "target", "base": BASE, "location": bytes}));
    }
    for byte in 0_u8..=255 {
        for prefix in [b"".as_slice(), b"/before/"] {
            let location: Vec<_> = prefix.iter().copied().chain([byte]).chain(*b"/after").collect();
            cases.push(json!({"kind": "target", "base": BASE, "location": location}));
        }
    }
    for headers in [
        json!([]),
        json!([["uri", b"/fallback".as_slice()]]),
        json!([["location", b"/first".as_slice()], ["location", b"/second".as_slice()]]),
        json!([["uri", b"/fallback".as_slice()], ["location", b"".as_slice()]]),
    ] {
        cases.push(case("GET", vec![json!({"status": 302, "headers": headers})]));
    }
    for count in 1..=12 {
        for distinct in [false, true] {
            for restart in [false, true] {
                let steps = (0..count)
                    .map(|index| {
                        let name = if distinct {
                            index
                        } else {
                            0
                        };
                        let mut step = step(302, &format!("/target-{name}"));
                        step["restart"] = json!(restart);
                        step
                    })
                    .collect();
                cases.push(case("POST", steps));
            }
        }
    }
    for url in ["http:///path", "https:///path", "http:/path", "https:relative", "http://"] {
        cases.push(json!({"kind": "missing_host", "base": url}));
    }
    for pattern in 0..4 {
        let steps = (0..40)
            .map(|index| {
                let location = match pattern {
                    0 => format!("/target-{}", index / 4),
                    1 => format!(
                        "/target-{}",
                        if index < 36 {
                            index % 9
                        } else {
                            9
                        }
                    ),
                    2 => format!("/target-{}", index % 10),
                    _ => format!("/same#fragment-{index}"),
                };
                step(302, &location)
            })
            .collect();
        cases.push(case("GET", steps));
    }
    assert_eq!(cases.len(), 927);
    let expected: Vec<_> = cases.iter().map(observe).collect();
    compare_source(&cases, &expected);
}

#[tokio::test]
async fn incomplete_redirect_bodies_fail_before_the_next_request() {
    use std::io::Read;
    use std::net::TcpListener;
    use std::time::Duration;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let worker = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut request = Vec::new();
        while !request.windows(4).any(|part| part == b"\r\n\r\n") {
            let mut bytes = [0; 1024];
            let count = stream.read(&mut bytes).unwrap();
            assert_ne!(count, 0);
            request.extend_from_slice(&bytes[..count]);
        }
        stream.write_all(b"HTTP/1.1 302 Fixture\r\nLocation: /next\r\nContent-Length: 20\r\nConnection: close\r\n\r\nshort").unwrap();
    });
    let client = Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .build()
        .unwrap();
    let request = client.get(format!("http://{address}/start")).build().unwrap();
    let mut history = History::default();
    let error = send(&client, request, &mut history).await.unwrap_err();
    worker.join().unwrap();
    assert!(matches!(error, Error::Http(error) if error.is_body() || error.is_decode()));
    assert_eq!(history.visits.get(&format!("http://{address}/next")), Some(&1));
}

#[tokio::test(start_paused = true)]
async fn missing_redirect_hosts_retry_the_original_request_without_host_repair() {
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let worker = tokio::spawn(async move {
        for _ in 0..4 {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            while !request.windows(4).any(|part| part == b"\r\n\r\n") {
                let mut bytes = [0; 1024];
                let count = stream.read(&mut bytes).await.unwrap();
                assert_ne!(count, 0);
                request.extend_from_slice(&bytes[..count]);
            }
            assert!(
                request.starts_with(b"GET /start HTTP/1.1\r\n"),
                "{}",
                String::from_utf8_lossy(&request)
            );
            // A same-scheme target inherits the original authority in urljoin.
            // Changing scheme preserves the missing authority until urlopen.
            stream.write_all(b"HTTP/1.1 302 Fixture\r\nLocation: https:///misrouted.invalid/next\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.unwrap();
        }
    });
    let client = Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .pool_max_idle_per_host(0)
        .build()
        .unwrap();
    let request = client.get(format!("http://{address}/start")).build().unwrap();
    let started = tokio::time::Instant::now();
    let error = super::super::retry::send(&client, request).await.unwrap_err();
    worker.await.unwrap();
    assert!(matches!(error, Error::GitHubUrl(message) if message.contains("no host given")));
    assert_eq!(started.elapsed(), Duration::from_secs(2 + 4 + 8));
}

fn step(status: u16, location: &str) -> Value {
    json!({"status": status, "headers": [["location", location.as_bytes()]]})
}

fn case(method: &str, steps: Vec<Value>) -> Value {
    json!({"kind": "sequence", "base": BASE, "method": method, "steps": steps})
}

fn request_headers() -> HeaderMap {
    HeaderMap::from_iter([
        (header::AUTHORIZATION, HeaderValue::from_static("Bearer fixture")),
        (header::CONTENT_TYPE, HeaderValue::from_static("application/json")),
        (header::CONTENT_LENGTH, HeaderValue::from_static("7")),
        (header::COOKIE, HeaderValue::from_static("session=fixture")),
        (HeaderName::from_static("x-fixture"), HeaderValue::from_static("retained")),
    ])
}

fn observe(case: &Value) -> Value {
    let base = case["base"].as_str().unwrap();
    if case["kind"] == "missing_host" {
        return json!({"no_host": matches!(target::transport_url(base), Err(Error::GitHubUrl(_)))});
    }
    if case["kind"] == "target" {
        let location: Vec<u8> = serde_json::from_value(case["location"].clone()).unwrap();
        return match target::resolve(base, &location) {
            Ok(Some(url)) => json!({"url": url}),
            Ok(None) => json!({"stop": 302}),
            Err(_) => json!({"error": true}),
        };
    }
    let original_method: Method = case["method"].as_str().unwrap().parse().unwrap();
    let mut method = original_method.clone();
    let mut current = base.to_owned();
    let mut headers = request_headers();
    let mut history = History::default();
    let mut events = Vec::new();
    for step in case["steps"].as_array().unwrap() {
        if step["restart"] == true {
            current = base.into();
            method = original_method.clone();
            headers = request_headers();
        }
        let status = step["status"].as_u64().unwrap() as u16;
        let mut response_headers = HeaderMap::new();
        for header in step["headers"].as_array().unwrap() {
            let name: HeaderName = header[0].as_str().unwrap().parse().unwrap();
            let bytes: Vec<u8> = serde_json::from_value(header[1].clone()).unwrap();
            response_headers.append(name, HeaderValue::from_bytes(&bytes).unwrap());
        }
        match history.next(&current, &method, &headers, status, &response_headers) {
            Ok(Some(next)) => {
                let fields: serde_json::Map<_, _> = next
                    .headers
                    .iter()
                    .map(|(name, value)| (name.to_string(), json!(value.to_str().unwrap())))
                    .collect();
                events.push(json!({"url": next.url, "method": next.method.as_str(), "headers": fields, "read": true, "close": true}));
                current = next.url;
                method = next.method;
                headers = next.headers;
            }
            result => {
                let mut event = if result.is_err() {
                    json!({"error": true})
                } else {
                    json!({"stop": status})
                };
                event["read"] = json!(false);
                event["close"] = json!(false);
                events.push(event);
                break;
            }
        }
    }
    json!(events)
}

fn compare_source(cases: &[Value], expected: &[Value]) {
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
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(cases).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(actual, expected, "case {index}: {}", cases[index]);
    }
    eprintln!("matched {} urllib redirect cases", cases.len());
}
