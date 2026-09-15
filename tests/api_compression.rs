//! Content decoding precedes JSON parsing and preserves streamed-download semantics.

#[path = "api_compression/fixture.rs"]
mod fixture;
#[path = "api_compression/status.rs"]
mod status;
mod support;

use std::fs;

use serde_json::json;
use sha2::{Digest, Sha256};
use support::{
    auth::*,
    http::{Response, Server},
    *,
};

use fixture::{ENCODINGS, response};

#[test]
fn json_methods_decode_compression_before_character_encoding() {
    for encoding in ENCODINGS {
        for operation in ["get", "put", "delete"] {
            let sandbox = Sandbox::new();
            write_config(&sandbox, &stored("key", "fixture-key"));
            let file = sandbox.path().join("payload");
            fs::write(&file, b"fixture").unwrap();
            let server = Server::start_with_headers(move |_, _| {
                let document = json!({"key":"fixture", "code":"café-🦀", "version":1,
                    "filename":"fixture", "url":null})
                .to_string();
                let body = document.encode_utf16().flat_map(u16::to_le_bytes).collect();
                response(
                    encoding,
                    Response {
                        status: 200,
                        content_type: "application/json; charset=windows-1252",
                        body,
                    },
                )
            });
            let mut args = vec![
                "share",
                operation,
                if operation == "put" {
                    file.to_str().unwrap()
                } else {
                    "fixture"
                },
            ];
            if operation == "put" {
                args.extend(["--acl", "private"]);
            }
            if operation == "delete" {
                args.push("--force");
            }
            let output = command(&sandbox, &server, &args).output().unwrap();
            assert_success(&output);
            if operation != "get" {
                assert!(String::from_utf8_lossy(&output.stdout).contains("café-🦀"));
            }
            let requests = server.requests();
            assert_eq!(
                requests.len(),
                if operation == "delete" {
                    2
                } else {
                    1
                }
            );
            assert!(requests.iter().all(|request| {
                request.headers.to_ascii_lowercase().contains("accept-encoding: gzip, deflate\r\n")
            }));
        }
    }
}

#[test]
fn identity_and_error_messages_decode_compressed_json() {
    for encoding in ENCODINGS {
        for status in [200, 500] {
            let sandbox = Sandbox::new();
            write_config(&sandbox, &stored("key", "fixture-key"));
            let server = Server::start_with_headers(move |_, _| {
                response(
                    encoding,
                    Response {
                        status,
                        ..Response::json(
                            json!({"email":"café@example.test", "message":"decoded failure"}),
                        )
                    },
                )
            });
            let mut cmd = if status == 200 {
                let mut cmd = command(&sandbox, &server, &["whoami"]);
                cmd.env("HCLI_API_KEY", "fixture-key");
                cmd
            } else {
                command(&sandbox, &server, &["share", "get", "fixture"])
            };
            let output = cmd.output().unwrap();
            assert_eq!(output.status.success(), status == 200);
            let report = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                report.contains(if status == 200 {
                    "café@example.test"
                } else {
                    "decoded failure"
                }),
                "{report}"
            );
        }
    }
}

#[test]
fn downloads_publish_decoded_bytes_and_hashes_and_preserve_head_wire_length() {
    for encoding in ENCODINGS {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let payload = b"fixture".repeat(20_000);
        let expected = payload.clone();
        let server = Server::start_with_headers(move |request, base| {
            if request.path == "/content" {
                response(
                    encoding,
                    Response {
                        status: 200,
                        content_type: "application/octet-stream",
                        body: payload.clone(),
                    },
                )
            } else {
                (
                    Response::json(
                        json!({"key":"fixture", "filename":"fixture", "size":payload.len(),
                    "url":format!("{base}/content")}),
                    ),
                    Vec::new(),
                )
            }
        });
        for _ in 0..2 {
            let output = command(&sandbox, &server, &["share", "get", "fixture"])
                .current_dir(sandbox.path())
                .output()
                .unwrap();
            assert_success(&output);
            assert_eq!(fs::read(sandbox.path().join("fixture")).unwrap(), expected);
            fs::remove_file(sandbox.path().join("fixture")).unwrap();
        }
        let cache = sandbox.path().join("cache/downloads/fixture");
        assert_eq!(fs::read(cache).unwrap(), expected);
        assert_eq!(
            fs::read_to_string(sandbox.path().join("cache/downloads/fixture.sha256")).unwrap(),
            format!("{:x}", Sha256::digest(&expected))
        );
        let methods: Vec<_> = server
            .requests()
            .into_iter()
            .filter(|request| request.path == "/content")
            .map(|request| request.method)
            .collect();
        // HTTPX retains the encoded Content-Length. Compressed cache entries
        // therefore miss the size check against their decoded on-disk length.
        assert_eq!(
            methods,
            if encoding == "unknown" {
                vec!["GET", "HEAD"]
            } else {
                vec!["GET", "HEAD", "GET"]
            }
        );
    }
}

#[test]
fn invalid_download_compression_preserves_existing_cache_and_output() {
    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("key", "fixture-key"));
    let cache = sandbox.path().join("cache/downloads");
    fs::create_dir_all(&cache).unwrap();
    fs::write(cache.join("fixture"), b"old cache").unwrap();
    fs::write(cache.join("fixture.sha256"), b"old checksum").unwrap();
    fs::write(sandbox.path().join("fixture"), b"old output").unwrap();
    let server = Server::start_with_headers(|request, base| {
        if request.path == "/content" {
            let mut body = fixture::encode("gzip", &b"decoded".repeat(20_000));
            let trailer = body.len() - 8;
            body[trailer] ^= 1;
            (
                Response {
                    status: 200,
                    content_type: "application/octet-stream",
                    body,
                },
                vec![("Content-Encoding".into(), "gzip".into())],
            )
        } else {
            (
                Response::json(json!({"key":"fixture", "filename":"fixture", "size":7,
                "url":format!("{base}/content")})),
                Vec::new(),
            )
        }
    });
    let output = command(&sandbox, &server, &["share", "get", "fixture", "--force"])
        .current_dir(sandbox.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("response decoding failed"));
    assert_eq!(fs::read(cache.join("fixture")).unwrap(), b"old cache");
    assert_eq!(fs::read(cache.join("fixture.sha256")).unwrap(), b"old checksum");
    assert_eq!(fs::read(sandbox.path().join("fixture")).unwrap(), b"old output");
    assert_eq!(fs::read_dir(cache).unwrap().count(), 2);
}

#[test]
fn uploads_decode_final_and_redirect_bodies_before_confirmation() {
    for status in [200, 302, 401, 500] {
        for invalid in [false, true] {
            let sandbox = Sandbox::new();
            write_config(&sandbox, &stored("key", "fixture-key"));
            let file = sandbox.path().join("payload");
            fs::write(&file, b"fixture").unwrap();
            let server = Server::start_with_headers(move |request, base| {
                if request.path == "/api/assets/shared" {
                    return (
                        Response::json(json!({"key":"fixture", "code":"fixture", "version":1,
                        "url":format!("{base}/signed")})),
                        Vec::new(),
                    );
                }
                if request.path != "/signed" {
                    return (Response::json(json!({})), Vec::new());
                }
                let (mut response, mut headers) = response(
                    "gzip",
                    Response {
                        status,
                        ..Response::json(json!({"message":"decoded upload failure"}))
                    },
                );
                if invalid {
                    response.body = b"broken gzip".to_vec();
                }
                if status == 302 {
                    headers.push(("Location".into(), "/redirected".into()));
                }
                (response, headers)
            });
            let output = command(
                &sandbox,
                &server,
                &["share", "put", file.to_str().unwrap(), "--acl", "private"],
            )
            .output()
            .unwrap();
            let success = status < 400 && !invalid;
            assert_eq!(output.status.success(), success);
            let requests = server.requests();
            assert_eq!(
                requests.len(),
                2 + usize::from(success) + usize::from(status == 302 && !invalid)
            );
            if invalid {
                assert!(
                    String::from_utf8_lossy(&output.stderr).contains("response decoding failed")
                );
            }
            if success {
                assert_eq!(requests.last().unwrap().path, "/api/assets/shared/fixture");
            }
        }
    }
}
