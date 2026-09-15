//! Arbitrary JSON error messages retain Python's display semantics at API boundaries.

mod support;

use std::fs;

use serde_json::json;
use support::{
    auth::*,
    http::{Response, Server},
    *,
};

const MESSAGES: [(&str, &str); 8] = [
    ("null", "None"),
    ("true", "True"),
    ("0", "0"),
    ("1e20", "1e+20"),
    ("-2.0156234722508763e14", "-201562347225087.62"),
    (r#"[null,true,"it's"]"#, "[None, True, \"it's\"]"),
    (r#"{"z":false,"a":"café-🦀"}"#, "{'z': False, 'a': 'café-🦀'}"),
    (r#""""#, ""),
];

fn failure(message: &str) -> Response {
    Response {
        status: 500,
        content_type: "application/json",
        body: format!("{{\"message\":{message}}}").into_bytes(),
    }
}

fn assert_message(output: &std::process::Output, expected: &str) {
    assert_eq!(output.status.code(), Some(1));
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(report.contains(&format!("API error (500): {expected}")), "{report}");
    assert!(!report.contains("API request failed"), "{report}");
}

#[test]
fn json_get_post_and_delete_preserve_non_string_error_messages() {
    for (message, expected) in MESSAGES {
        for operation in ["get", "put", "delete"] {
            let sandbox = Sandbox::new();
            write_config(&sandbox, &stored("key", "fixture-key"));
            let file = sandbox.path().join("payload");
            fs::write(&file, b"fixture").unwrap();
            let server = Server::start(move |request, _| {
                if operation == "delete" && request.method == "GET" {
                    Response::json(json!({"key":"fixture", "filename":"fixture", "size":7}))
                } else {
                    failure(message)
                }
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
            assert_message(&output, expected);
            let requests = server.requests();
            assert_eq!(
                requests.len(),
                if operation == "delete" {
                    2
                } else {
                    1
                }
            );
            assert_eq!(
                requests.last().unwrap().method,
                match operation {
                    "get" => "GET",
                    "put" => "POST",
                    _ => "DELETE",
                }
            );
        }
    }
}

#[test]
fn signed_put_errors_keep_the_message_and_prevent_confirmation() {
    for (message, expected) in MESSAGES {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let file = sandbox.path().join("payload");
        fs::write(&file, b"fixture").unwrap();
        let server = Server::start(move |request, base| {
            if request.path == "/api/assets/shared" {
                Response::json(json!({"key":"fixture", "code":"fixture", "version":1,
                    "url":format!("{base}/signed")}))
            } else {
                failure(message)
            }
        });
        let output = command(
            &sandbox,
            &server,
            &["share", "put", file.to_str().unwrap(), "--acl", "private"],
        )
        .output()
        .unwrap();
        assert_message(&output, expected);
        let requests = server.requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[1].method, "PUT");
        assert_eq!(requests[1].body, b"fixture");
    }
}
