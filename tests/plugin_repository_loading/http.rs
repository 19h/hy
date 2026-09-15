//! Exercise the production client, body decoder and repository command together.

use std::io::Write;

use flate2::{Compression, write::GzEncoder};
use serde_json::{Value, json};

use crate::support::http::{Response, Server};
use crate::support::{Sandbox, assert_success};

fn document() -> Vec<u8> {
    serde_json::to_vec(&json!({"version": 1, "plugins": []})).unwrap()
}

fn reply(status: u16, body: Vec<u8>) -> Response {
    Response {
        status,
        content_type: "application/json",
        body,
    }
}

fn gzip(body: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(body).unwrap();
    encoder.finish().unwrap()
}

fn snapshot(sandbox: &Sandbox, url: &str) -> std::process::Output {
    sandbox
        .command(&["plugin", "--repo", url, "repo", "snapshot"])
        .env("HCLI_API_KEY", "fixture-key")
        .env("GITHUB_TOKEN", "fixture-github-token")
        .output()
        .unwrap()
}

#[test]
fn only_supported_redirect_statuses_with_locations_are_followed() {
    let sandbox = Sandbox::new();
    for status in [200, 300, 301, 302, 303, 304, 305, 306, 307, 308, 399, 401, 403, 404, 500] {
        for has_location in [false, true] {
            let server = Server::start_with_headers(move |request, _| {
                if request.path == "/final" {
                    return (reply(200, document()), Vec::new());
                }
                let headers = if has_location {
                    vec![("Location".into(), "/final".into())]
                } else {
                    Vec::new()
                };
                (reply(status, document()), headers)
            });
            let output = snapshot(&sandbox, &format!("{}/start", server.url));
            let followed = has_location && matches!(status, 301 | 302 | 303 | 307 | 308);
            assert_eq!(output.status.success(), status == 200 || followed, "{status}: {output:?}");
            let requests = server.requests();
            assert_eq!(
                requests.len(),
                if followed {
                    2
                } else {
                    1
                }
            );
            if followed {
                assert_eq!(requests[1].path, "/final");
            }
            for request in requests {
                let headers = request.headers.to_ascii_lowercase();
                assert!(headers.contains("accept-encoding: gzip, deflate\r\n"));
                assert!(!headers.contains("x-api-key:"));
                assert!(!headers.contains("authorization:"));
            }
        }
    }
}

#[test]
fn compressed_bodies_are_decoded_before_status_and_redirect_handling() {
    let sandbox = Sandbox::new();
    for status in [200, 302, 401] {
        for valid in [false, true] {
            let server = Server::start_with_headers(move |request, _| {
                if request.path == "/final" {
                    return (reply(200, document()), Vec::new());
                }
                let body = if valid {
                    gzip(&document())
                } else {
                    b"invalid gzip".to_vec()
                };
                (
                    reply(status, body),
                    vec![
                        ("Content-Encoding".into(), "gzip".into()),
                        ("Location".into(), "/final".into()),
                    ],
                )
            });
            let output = snapshot(&sandbox, &format!("{}/start", server.url));
            assert_eq!(output.status.success(), valid && status != 401, "{status}: {output:?}");
            assert_eq!(
                server.requests().len(),
                if valid && status == 302 {
                    2
                } else {
                    1
                }
            );
            if !valid {
                assert!(
                    String::from_utf8_lossy(&output.stderr).contains("response decoding failed")
                );
            }
        }
    }
}

#[test]
fn redirect_cookie_session_applies_path_and_secure_rules() {
    let sandbox = Sandbox::new();
    let server = Server::start_with_headers(|request, _| match request.path.as_str() {
        "/start" => (
            reply(302, Vec::new()),
            vec![
                ("Location".into(), "/private/next".into()),
                ("Set-Cookie".into(), "session=fixture; Path=/".into()),
                ("Set-Cookie".into(), "scoped=private; Path=/private".into()),
                ("Set-Cookie".into(), "secure=secret; Path=/; Secure".into()),
            ],
        ),
        "/private/next" => (reply(307, Vec::new()), vec![("Location".into(), "/final".into())]),
        "/final" => (reply(200, document()), Vec::new()),
        _ => (Response::missing(), Vec::new()),
    });
    let output = snapshot(&sandbox, &format!("{}/start", server.url));
    assert_success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!({"version": 1, "plugins": []})
    );
    let requests = server.requests();
    assert_eq!(requests.len(), 3);
    let cookies: Vec<_> = requests
        .iter()
        .map(|request| {
            request.headers.lines().find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("cookie").then(|| value.trim())
            })
        })
        .collect();
    assert_eq!(cookies, [None, Some("scoped=private; session=fixture"), Some("session=fixture")]);
}
