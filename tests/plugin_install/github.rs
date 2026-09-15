//! Direct release acquisition through owned HTTP servers and the public CLI.

use std::io::Write;

use flate2::{Compression, write::GzEncoder};

use super::*;
use support::http::{Response, Server};

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

fn install(sandbox: &Sandbox, server: &Server) -> std::process::Output {
    sandbox
        .command(&["plugin", "install", "https://github.com/o/r"])
        .env("GITHUB_API_URL", &server.url)
        .env("GITHUB_TOKEN", "fixture-github-token")
        .env("HCLI_API_KEY", "fixture-api-key")
        .output()
        .unwrap()
}

#[test]
fn release_selection_accepts_irrelevant_assets_sizes_and_json_byte_encodings() {
    for (size, utf16) in [(json!(-1), false), (json!(0.5), true), (json!(true), false)] {
        let sandbox = Sandbox::new();
        let path = sandbox.path().join("plugin.zip");
        archive(&path, "1", &[]);
        let bytes = fs::read(path).unwrap();
        let server = Server::start_with_headers(move |request, base| {
            let body = if request.path == "/repos/o/r/releases/latest" {
                let text = json!({"assets": [
                    {"name": "README", "size": "not inspected"}, {},
                    {"name": "plugin.ZIP", "browser_download_url": format!("{base}/asset"), "size": size},
                ]}).to_string();
                if utf16 {
                    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
                } else {
                    text.into_bytes()
                }
            } else {
                bytes.clone()
            };
            (Response::zip(gzip(&body)), vec![("Content-Encoding".into(), "gzip".into())])
        });
        let output = install(&sandbox, &server);
        assert_success(&output);
        assert!(sandbox.path().join("idausr/plugins/example/plugin.py").is_file());
        let requests = server.requests();
        assert_eq!(requests.len(), 2);
        assert!(requests[0].headers.contains("accept: application/vnd.github.v3+json\r\n"));
        assert!(requests[1].headers.contains("accept: */*\r\n"));
        for request in requests {
            let headers = request.headers.to_ascii_lowercase();
            assert!(!headers.contains("authorization:"));
            assert!(!headers.contains("x-api-key:"));
        }
    }
}

#[test]
fn twenty_redirects_are_allowed_and_metadata_cookies_do_not_reach_the_asset_operation() {
    for count in [20, 21] {
        let sandbox = Sandbox::new();
        let path = sandbox.path().join("plugin.zip");
        archive(&path, "1", &[]);
        let bytes = fs::read(path).unwrap();
        let server = Server::start_with_headers(move |request, base| {
            if request.path == "/asset" {
                assert!(!request.headers.to_ascii_lowercase().contains("cookie:"));
                return (
                    Response {
                        status: 302,
                        content_type: "text/plain",
                        body: Vec::new(),
                    },
                    vec![
                        ("Location".into(), "/asset-final".into()),
                        ("Set-Cookie".into(), "asset=fixture; Path=/".into()),
                    ],
                );
            }
            if request.path == "/asset-final" {
                assert!(request.headers.contains("cookie: asset=fixture\r\n"));
                return (Response::zip(bytes.clone()), Vec::new());
            }
            let index = request
                .path
                .strip_prefix("/meta/")
                .map_or(0, |value| value.parse::<usize>().unwrap());
            if index == count {
                assert!(request.headers.contains("cookie: metadata=fixture\r\n"));
                return (
                    Response::json(
                        json!({"assets": [{"name": "a.zip", "browser_download_url": format!("{base}/asset")}]}),
                    ),
                    Vec::new(),
                );
            }
            (
                Response {
                    status: 302,
                    content_type: "text/plain",
                    body: Vec::new(),
                },
                vec![
                    ("Location".into(), format!("/meta/{}", index + 1)),
                    ("Set-Cookie".into(), "metadata=fixture; Path=/".into()),
                ],
            )
        });
        let output = install(&sandbox, &server);
        assert_eq!(output.status.success(), count == 20, "{output:?}");
        if count == 20 {
            assert_eq!(server.requests().len(), 23);
        } else {
            assert_eq!(server.requests().len(), 21);
            assert!(
                String::from_utf8_lossy(&output.stderr)
                    .contains("Exceeded maximum allowed redirects")
            );
        }
    }
}

#[test]
fn selection_and_compressed_response_failures_prevent_asset_fetch_and_installation() {
    for (document, expected) in [
        (json!({}), "No .zip asset found in release (latest) for o/r"),
        (
            json!({"assets": [{"name": "one.zip"}, {"name": "two.ZIP"}]}),
            "Multiple .zip assets found in release: one.zip, two.ZIP. Cannot determine which to install.",
        ),
        (
            json!({"assets": [{"name": "a.zip", "size": 104857601, "browser_download_url": "https://mirror.test/a"}]}),
            "Asset a.zip (104857601 bytes) exceeds maximum size limit (104857600 bytes)",
        ),
    ] {
        let sandbox = Sandbox::new();
        let server = Server::start(move |_, _| Response::json(document.clone()));
        let output = install(&sandbox, &server);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(expected), "{output:?}");
        assert_eq!(server.requests().len(), 1);
        assert!(!sandbox.path().join("idausr/plugins/example").exists());
    }
    for status in [200, 302, 403] {
        let sandbox = Sandbox::new();
        let server = Server::start_with_headers(move |_, _| {
            (
                Response {
                    status,
                    content_type: "application/json",
                    body: b"invalid gzip".to_vec(),
                },
                vec![
                    ("Content-Encoding".into(), "gzip".into()),
                    ("Location".into(), "/next".into()),
                ],
            )
        });
        let output = install(&sandbox, &server);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("response decoding failed"));
        assert_eq!(server.requests().len(), 1);
    }
}

#[test]
fn nonfinite_sizes_and_surrogate_metadata_follow_python_selection_through_installation() {
    for size in ["NaN", "-Infinity", "Infinity"] {
        for utf16 in [false, true] {
            let sandbox = Sandbox::new();
            let path = sandbox.path().join("plugin.zip");
            archive(&path, "1", &[]);
            let bytes = fs::read(path).unwrap();
            let server = Server::start(move |request, base| {
                if request.path == "/asset" {
                    return Response::zip(bytes.clone());
                }
                let nested = format!("{}\"\\ud800\"{}", "[".repeat(1100), "]".repeat(1100));
                let document = format!(
                    r#"{{"assets":[{{"name":"\udfff.ZIP","size":{size},
                    "browser_download_url":"{base}/asset"}}],"unused":{nested}}}"#,
                );
                let body = if utf16 {
                    document.encode_utf16().flat_map(u16::to_le_bytes).collect()
                } else {
                    document.into_bytes()
                };
                Response {
                    status: 200,
                    content_type: "application/json",
                    body,
                }
            });
            let output = install(&sandbox, &server);
            if size == "Infinity" {
                assert!(!output.status.success());
                let error = String::from_utf8_lossy(&output.stderr);
                assert!(
                    error.contains(
                        "Asset \\udfff.ZIP (inf bytes) exceeds maximum size limit (104857600 bytes)"
                    ),
                    "{error}"
                );
                assert_eq!(server.requests().len(), 1);
                assert!(!sandbox.path().join("idausr/plugins/example").exists());
            } else {
                assert_success(&output);
                assert_eq!(server.requests().len(), 2);
                assert!(sandbox.path().join("idausr/plugins/example/plugin.py").is_file());
            }
        }
    }
}
