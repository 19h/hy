//! Compare the source consumers' HTTP error text and body-read boundaries.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use serde_json::{Value, json};

use super::*;

#[tokio::test]
async fn http_failures_match_source_consumers_and_reason_decoding() {
    let mut cases = Vec::new();
    let bodies: &[&[u8]] = &[
        b"",
        b"access denied",
        br#"{"message":"token rejected"}"#,
        "拒否 🧠".as_bytes(),
        b"a\r\nb\rc",
        b"\xef\xbb\xbfBOM",
        b"x\0tail",
        b"\xff",
        b"x\xe2\x82",
        b"\xed\xa0\x80",
    ];
    for status in [300, 304, 307, 400, 403, 404, 429, 500, 503, 599] {
        for consumer in ["graphql", "search", "asset", "source"] {
            for body in bodies {
                cases.push(json!({"status": status, "consumer": consumer, "body": body, "reason": b"Fixture".as_slice(), "broken": false}));
            }
            cases.push(json!({"status": status, "consumer": consumer, "body": [], "reason": b"Fixture".as_slice(), "broken": true}));
        }
    }
    for byte in std::iter::once(b'\t').chain(32..=126).chain(128..=255) {
        for reason in [
            [vec![byte], b"Fixture".to_vec(), vec![byte]].concat(),
            [b"Before".to_vec(), vec![byte], b"After".to_vec()].concat(),
        ] {
            cases.push(json!({"status": 401, "consumer": "search", "body": [255], "reason": reason, "broken": false}));
        }
    }
    assert_eq!(cases.len(), 888);
    let mut expected = Vec::new();
    for case in &cases {
        expected.push(observe(case).await);
    }
    compare_source(&cases, &expected);
}

#[tokio::test]
async fn wire_reasons_and_incomplete_error_bodies_keep_consumer_boundaries() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    for (phrase, expected_reason) in [
        (b"Unauthorized".as_slice(), "Unauthorized"),
        (b" Gr\xe4nse ", "Gränse"),
        (b"", ""),
        (b"\x85Fixture\xa0", "Fixture"),
    ] {
        for graphql in [false, true] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let mut wire = b"HTTP/1.1 401 ".to_vec();
            wire.extend_from_slice(phrase);
            wire.extend_from_slice(b"\r\nContent-Length: 20\r\nConnection: close\r\n\r\nshort");
            let worker = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                while !request.windows(4).any(|part| part == b"\r\n\r\n") {
                    let mut bytes = [0; 1024];
                    let count = stream.read(&mut bytes).await.unwrap();
                    assert_ne!(count, 0);
                    request.extend_from_slice(&bytes[..count]);
                }
                stream.write_all(&wire).await.unwrap();
            });
            let client =
                Client::builder().no_proxy().retry(reqwest::retry::never()).build().unwrap();
            let response = client.get(format!("http://{address}/fixture")).send().await.unwrap();
            assert_eq!(reason(&response), expected_reason);
            let error = if graphql {
                read_graphql_json(response).await.unwrap_err()
            } else {
                read_search_json(response).await.unwrap_err()
            };
            worker.await.unwrap();
            if graphql {
                assert!(
                    matches!(error, Error::Http(error) if error.is_body() || error.is_decode())
                );
            } else {
                assert_eq!(error.to_string(), format!("HTTP Error 401: {expected_reason}"));
            }
        }
    }
}

async fn observe(case: &Value) -> Value {
    let read = Arc::new(AtomicBool::new(false));
    let observed_read = Arc::clone(&read);
    let bytes: Vec<u8> = serde_json::from_value(case["body"].clone()).unwrap();
    let broken = case["broken"].as_bool().unwrap();
    let stream = futures_util::stream::once(async move {
        observed_read.store(true, Ordering::SeqCst);
        if broken {
            Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "fixture incomplete body"))
        } else {
            Ok(bytes)
        }
    });
    let mut response = hyper::Response::builder()
        .status(case["status"].as_u64().unwrap() as u16)
        .body(reqwest::Body::wrap_stream(stream))
        .unwrap();
    let reason: Vec<u8> = serde_json::from_value(case["reason"].clone()).unwrap();
    response.extensions_mut().insert(hyper::ext::ReasonPhrase::try_from(reason).unwrap());
    let response: Response = response.into();
    let result = match case["consumer"].as_str().unwrap() {
        "graphql" => read_graphql_json(response).await.map(|_| ()),
        "search" => read_search_json(response).await.map(|_| ()),
        "asset" | "source" => require_success(&response),
        _ => unreachable!(),
    };
    let mut outcome = match result {
        Err(Error::Other(message)) => json!({"kind": "http", "message": message}),
        Err(Error::UnicodeDecode(message)) => json!({"kind": "decode", "message": message}),
        Err(Error::Http(_)) => json!({"kind": "read"}),
        result => panic!("unexpected HTTP error outcome: {result:?}"),
    };
    outcome["read"] = json!(read.load(Ordering::SeqCst));
    outcome
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
    eprintln!("matched {} upstream HTTP failure cases", cases.len());
}
