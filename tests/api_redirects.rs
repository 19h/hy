//! API JSON requests and streamed file transfers have distinct redirect policies.

#[path = "api_redirects/reference.rs"]
mod reference;
mod support;

use std::fs;

use serde_json::json;
use support::{
    auth::*,
    http::{Response, Server},
    *,
};

#[test]
fn streamed_put_redirects_never_confirm_an_unreplayable_body() {
    let mut cases = Vec::new();
    for status in [200, 201, 300, 301, 302, 303, 304, 307, 308, 400, 500] {
        for location in [false, true] {
            let sandbox = Sandbox::new();
            write_config(&sandbox, &stored("key", "fixture-key"));
            let file = sandbox.path().join("payload.json");
            fs::write(&file, b"fixture").unwrap();
            let server = Server::start_with_headers(move |request, base| {
                let mut headers = Vec::new();
                let response = match request.path.as_str() {
                    "/api/assets/shared" => Response::json(json!({
                        "key": "fixture", "code": "fixture", "version": 1,
                        "url": format!("{base}/signed-put"),
                    })),
                    "/signed-put" => {
                        if location {
                            headers.push(("Location".into(), "/redirected".into()));
                        }
                        Response {
                            status,
                            ..Response::json(json!({}))
                        }
                    }
                    _ => Response::json(json!({})),
                };
                (response, headers)
            });
            let output = command(
                &sandbox,
                &server,
                &["share", "put", file.to_str().unwrap(), "--acl", "private"],
            )
            .output()
            .unwrap();
            let success = status < 400 && !(location && matches!(status, 301 | 307 | 308));
            assert_eq!(
                output.status.success(),
                success,
                "{status}, location={location}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let requests = server.requests();
            assert_eq!(requests[1].method, "PUT");
            assert_eq!(requests[1].body, b"fixture");
            let redirected = location && matches!(status, 302 | 303);
            assert_eq!(requests.len(), 2 + usize::from(redirected) + usize::from(success));
            if redirected {
                assert_eq!(requests[2].method, "GET");
                assert!(requests[2].body.is_empty());
                let headers = requests[2].headers.to_ascii_lowercase();
                assert!(headers.contains("content-type: application/json"));
                assert!(!headers.contains("content-length:"));
            }
            if success {
                assert_eq!(requests.last().unwrap().path, "/api/assets/shared/fixture");
            }
            let events: Vec<_> = requests
                .iter()
                .filter(|request| matches!(request.path.as_str(), "/signed-put" | "/redirected"))
                .map(|request| json!([request.method, request.path]))
                .collect();
            cases.push(json!({"status": status, "location": location,
                "expected": {"success": success, "events": events}}));
        }
    }
    reference::compare(&cases);
}

#[test]
fn file_transfers_allow_twenty_redirects_and_reject_the_twenty_first() {
    for redirects in [19, 20, 21] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let file = sandbox.path().join("payload");
        fs::write(&file, b"fixture").unwrap();
        let server = Server::start_with_headers(move |request, base| {
            if request.path == "/api/assets/shared" {
                return (
                    Response::json(json!({"key":"fixture", "code":"fixture", "version":1,
                    "url":format!("{base}/hop/0")})),
                    Vec::new(),
                );
            }
            if let Some(hop) =
                request.path.strip_prefix("/hop/").and_then(|value| value.parse::<usize>().ok())
                && hop < redirects
            {
                return (
                    Response {
                        status: 302,
                        ..Response::json(json!({}))
                    },
                    vec![("Location".into(), format!("/hop/{}", hop + 1))],
                );
            }
            (Response::json(json!({})), Vec::new())
        });
        let output = command(
            &sandbox,
            &server,
            &["share", "put", file.to_str().unwrap(), "--acl", "private"],
        )
        .output()
        .unwrap();
        assert_eq!(output.status.success(), redirects <= 20);
        let requests = server.requests();
        assert_eq!(
            requests.len(),
            if redirects <= 20 {
                redirects + 3
            } else {
                22
            }
        );
        assert_eq!(requests.iter().filter(|request| request.method == "PUT").count(), 1);
    }
}

#[test]
fn json_requests_parse_redirect_responses_without_following_location() {
    for operation in ["get", "delete", "put"] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let file = sandbox.path().join("payload");
        fs::write(&file, b"fixture").unwrap();
        let server = Server::start_with_headers(|_, _| {
            (
                Response {
                    status: 302,
                    ..Response::json(json!({
                        "key":"fixture", "code":"fixture", "version":1, "filename":"fixture", "url":null,
                    }))
                },
                vec![("Location".into(), "/must-not-follow".into())],
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
        let requests = server.requests();
        assert_eq!(
            requests.len(),
            if operation == "delete" {
                2
            } else {
                1
            }
        );
        assert!(requests.iter().all(|request| request.path != "/must-not-follow"));
    }
}

#[test]
fn a_truncated_upload_response_fails_before_confirmation() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let upload_url = format!("http://{}/upload", listener.local_addr().unwrap());
    let worker = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"fixture") {
            let mut buffer = [0; 1024];
            let count = socket.read(&mut buffer).unwrap();
            assert_ne!(count, 0);
            request.extend_from_slice(&buffer[..count]);
            assert!(request.len() < 65536);
        }
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\nx")
            .unwrap();
    });
    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("key", "fixture-key"));
    let file = sandbox.path().join("payload");
    fs::write(&file, b"fixture").unwrap();
    let server = Server::start(move |_, _| {
        Response::json(json!({
            "key":"fixture", "code":"fixture", "version":1, "url":upload_url,
        }))
    });
    let output =
        command(&sandbox, &server, &["share", "put", file.to_str().unwrap(), "--acl", "private"])
            .output()
            .unwrap();
    worker.join().unwrap();
    assert!(!output.status.success());
    assert_eq!(server.requests().len(), 1);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("uploaded successfully"));
}

#[test]
fn download_redirects_preserve_api_key_headers_and_publish_final_bytes() {
    let destination = Server::start(|_, _| Response {
        body: b"fixture".to_vec(),
        ..Response::json(json!({}))
    });
    let target = format!("{}/final", destination.url);
    let server = Server::start_with_headers(move |request, base| {
        if request.path.starts_with("/api/assets/s/") {
            return (
                Response::json(json!({"key":"fixture", "filename":"fixture.bin", "size":7,
                "url":format!("{base}/redirect")})),
                Vec::new(),
            );
        }
        (
            Response {
                status: 302,
                ..Response::json(json!({}))
            },
            vec![("Location".into(), target.clone())],
        )
    });
    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("key", "fixture-key"));
    let output = command(&sandbox, &server, &["share", "get", "fixture"])
        .current_dir(sandbox.path())
        .output()
        .unwrap();
    assert_success(&output);
    assert_eq!(fs::read(sandbox.path().join("fixture.bin")).unwrap(), b"fixture");
    let requests = destination.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert!(requests[0].headers.to_ascii_lowercase().contains("x-api-key: fixture-key"));
}

#[test]
fn cache_head_checks_accept_terminal_statuses_below_four_hundred() {
    for status in [200, 204, 301, 302, 304, 404] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let server = Server::start(move |request, base| {
            if request.path.starts_with("/api/assets/s/") {
                return Response::json(json!({"key":"fixture", "filename":"fixture.bin", "size":7,
                    "url":format!("{base}/bytes")}));
            }
            Response {
                status: if request.method == "HEAD" {
                    status
                } else {
                    200
                },
                body: b"fixture".to_vec(),
                ..Response::json(json!({}))
            }
        });
        for iteration in 0..2 {
            let output = command(&sandbox, &server, &["share", "get", "fixture"])
                .current_dir(sandbox.path())
                .output()
                .unwrap();
            assert_success(&output);
            let target = sandbox.path().join("fixture.bin");
            assert_eq!(fs::read(&target).unwrap(), b"fixture");
            if iteration == 0 {
                fs::remove_file(target).unwrap();
            }
        }
        let requests = server.requests();
        assert_eq!(requests.iter().filter(|request| request.method == "HEAD").count(), 1);
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.method == "GET" && request.path == "/bytes")
                .count(),
            if status < 400 {
                1
            } else {
                2
            },
            "{status}"
        );
    }
}
