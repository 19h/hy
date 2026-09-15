//! Cookie state spans API JSON, signed transfers, redirects and failed responses.

mod support;

use std::fs;

use serde_json::json;
use support::{
    auth::*,
    http::{Request, Response, Server},
    *,
};

fn cookie(request: &Request) -> Option<&str> {
    request
        .headers
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("cookie"))
        .map(|(_, value)| value.trim())
}

#[test]
fn upload_cookie_scope_and_updates_span_ticket_put_and_confirmation() {
    for different_host in [false, true] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let file = sandbox.path().join("payload");
        fs::write(&file, b"fixture").unwrap();
        let upload = Server::start_with_headers(|_, _| {
            (
                Response::json(json!({})),
                vec![
                    ("Set-Cookie".into(), "sid=updated; Path=/".into()),
                    ("Set-Cookie".into(), "put=done; Path=/".into()),
                ],
            )
        });
        let upload_url = format!(
            "{}/signed/put",
            if different_host {
                upload.url.replace("127.0.0.1", "localhost")
            } else {
                upload.url.clone()
            }
        );
        let server = Server::start_with_headers(move |request, _| {
            if request.path == "/api/assets/shared" {
                return (
                    Response::json(
                        json!({"key":"fixture", "code":"fixture", "version":1, "url":upload_url}),
                    ),
                    [
                        "sid=initial; Path=/",
                        "api=allowed; Path=/api",
                        "upload=allowed; Path=/signed",
                        "secure=hidden; Secure; Path=/",
                        "other=hidden; Domain=other.test; Path=/",
                    ]
                    .map(|value| ("Set-Cookie".into(), value.into()))
                    .to_vec(),
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
        assert_success(&output);
        let api = server.requests();
        let signed = upload.requests();
        assert_eq!(api.len(), 2);
        assert_eq!(signed.len(), 1);
        assert_eq!(cookie(&api[0]), None);
        assert_eq!(
            cookie(&signed[0]),
            if different_host {
                None
            } else {
                Some("upload=allowed; sid=initial")
            }
        );
        assert_eq!(
            cookie(&api[1]),
            Some(if different_host {
                "api=allowed; sid=initial"
            } else {
                "api=allowed; sid=updated; put=done"
            })
        );
    }
}

#[test]
fn redirects_refresh_cookie_headers_and_new_processes_start_empty() {
    let server = Server::start_with_headers(|request, base| {
        let (response, headers) = match request.path.as_str() {
            "/start" => (
                Response {
                    status: 302,
                    ..Response::json(json!({}))
                },
                vec![
                    ("Location", "/finish"),
                    ("Set-Cookie", "sid=redirect; Path=/"),
                    ("Set-Cookie", "step=next; Path=/finish"),
                ],
            ),
            "/finish" => (
                Response {
                    body: b"fixture".to_vec(),
                    ..Response::json(json!({}))
                },
                vec![("Set-Cookie", "sid=final; Path=/")],
            ),
            _ => (
                Response::json(json!({"key":"fixture", "filename":"fixture", "size":7,
                "url":format!("{base}/start")})),
                vec![("Set-Cookie", "sid=lookup; Path=/")],
            ),
        };
        (response, headers.into_iter().map(|(name, value)| (name.into(), value.into())).collect())
    });
    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("key", "fixture-key"));
    for _ in 0..2 {
        let output = command(&sandbox, &server, &["share", "get", "fixture", "--force"])
            .current_dir(sandbox.path())
            .output()
            .unwrap();
        assert_success(&output);
        assert_eq!(fs::read(sandbox.path().join("fixture")).unwrap(), b"fixture");
    }
    let requests = server.requests();
    assert_eq!(requests.len(), 6);
    for batch in requests.as_chunks::<3>().0 {
        assert_eq!(cookie(&batch[0]), None);
        assert_eq!(cookie(&batch[1]), Some("sid=lookup"));
        assert_eq!(cookie(&batch[2]), Some("step=next; sid=redirect"));
    }
}

#[cfg(unix)]
#[test]
fn failed_responses_update_batch_cookies_and_expiry_removes_them() {
    use support::terminal::Terminal;
    for status in [200, 401] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let server = Server::start_with_headers(move |request, _| {
            let (response, cookie) = match request.path.as_str() {
                "/api/assets/shared/first" => (
                    Response {
                        status,
                        body: b"invalid JSON".to_vec(),
                        ..Response::json(json!({}))
                    },
                    "sid=failed; Path=/",
                ),
                "/api/assets/shared/second" => {
                    (Response::json(json!({})), "sid=expired; Max-Age=0; Path=/")
                }
                "/api/assets/shared/third" => (Response::json(json!({})), ""),
                _ => (
                    Response::json(json!({"offset":0, "limit":100, "total":3,
                    "items":[{"key":"first","filename":"first"},{"key":"second","filename":"second"},{"key":"third","filename":"third"}]})),
                    "sid=list; Path=/",
                ),
            };
            (response, vec![("Set-Cookie".into(), cookie.into())])
        });
        let mut terminal = Terminal::start(command(&sandbox, &server, &["share", "list"]));
        terminal.wait_for("Select files to manage");
        terminal.send("\x01\r");
        terminal.wait_for("What would you like to do?");
        terminal.send("\r");
        terminal.wait_for("Are you sure");
        terminal.send("y\r");
        let (exit, output) = terminal.finish();
        assert!(exit.success(), "{output}");
        assert!(output.contains("Failed to delete first"), "{output}");
        let requests = server.requests();
        assert_eq!(requests.len(), 4);
        assert_eq!(cookie(&requests[1]), Some("sid=list"));
        assert_eq!(cookie(&requests[2]), Some("sid=failed"));
        assert_eq!(cookie(&requests[3]), None);
    }
}

#[test]
fn unencodable_cookie_headers_fail_before_the_next_request() {
    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("key", "fixture-key"));
    let server = Server::start_with_headers(|_, base| {
        (
            Response::json(json!({
                "key":"fixture", "filename":"fixture", "url":format!("{base}/bytes"),
            })),
            vec![("Set-Cookie".into(), "sid=é; Path=/".into())],
        )
    });
    let output = command(&sandbox, &server, &["share", "get", "fixture"])
        .current_dir(sandbox.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(server.requests().len(), 1);
    assert!(!sandbox.path().join("fixture").exists());
    assert!(String::from_utf8_lossy(&output.stdout).contains("cookie header contains non-ASCII"));
}
