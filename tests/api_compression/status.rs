use serde_json::json;

use crate::fixture::response;
use crate::support::{
    auth::*,
    http::{Response, Server},
    *,
};

#[path = "status/reference.rs"]
mod reference;

#[test]
fn buffered_and_streamed_errors_preserve_upstream_decoding_order() {
    let mut cases = Vec::new();
    for status in [401, 403, 404, 429, 500] {
        for invalid in [false, true] {
            for streamed in [false, true] {
                let sandbox = Sandbox::new();
                write_config(&sandbox, &stored("key", "fixture-key"));
                let server = Server::start_with_headers(move |request, base| {
                    if streamed && request.path != "/content" {
                        return (
                            Response::json(json!({"key":"fixture", "filename":"fixture", "size":7,
                            "url":format!("{base}/content")})),
                            Vec::new(),
                        );
                    }
                    let (mut response, headers) = response(
                        "gzip",
                        Response {
                            status,
                            ..Response::json(json!({"message":"decoded failure"}))
                        },
                    );
                    if invalid {
                        response.body = b"broken gzip".to_vec();
                    }
                    (response, headers)
                });
                let output = command(&sandbox, &server, &["share", "get", "fixture"])
                    .current_dir(sandbox.path())
                    .output()
                    .unwrap();
                assert!(!output.status.success());
                let expected = if invalid && !streamed {
                    "decoding failed"
                } else {
                    match status {
                        401 => "Authentication failed",
                        403 => "Access forbidden",
                        404 => "Resource not found",
                        429 => "Rate limit exceeded",
                        _ if streamed => "API request failed: 500",
                        _ => "decoded failure",
                    }
                };
                let report = String::from_utf8_lossy(&output.stdout);
                assert!(
                    report.contains(expected),
                    "{status}, invalid={invalid}, streamed={streamed}: {report}"
                );
                assert_eq!(
                    server.requests().len(),
                    if streamed {
                        2
                    } else {
                        1
                    }
                );
                cases.push(json!({"status":status, "invalid":invalid, "streamed":streamed, "expected":expected}));
            }
        }
    }
    reference::compare(&cases);
}
