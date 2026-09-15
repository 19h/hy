//! Upload contract tests: payloads, signed transfers, confirmation, and failure boundaries.

#[path = "asset_upload/reference.rs"]
mod reference;
mod support;
#[path = "asset_upload/tickets.rs"]
mod tickets;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use support::{
    auth::*,
    http::{Response, Server},
    *,
};

fn bucket() -> Value {
    json!({"filename": "fixture", "metadata": {"name": "Fixture"},
        "requiredMetadata": {"version": {"description": "Version", "example": "9.4"}}})
}

fn ticket(base: &str) -> Value {
    json!({"key": "///nested/file", "code": "fixture-code", "version": 7,
        "url": format!("{base}/signed-put"), "download_url": "https://invalid.test/ignored"})
}

#[test]
fn share_permissions_and_upload_lifecycle_match_the_upstream_contract() {
    for acl in ["private", "domain", "authenticated"] {
        let sandbox = Sandbox::new();
        let mut config = stored("key", "fixture-key");
        config["hcli.credentials"]["credentials"]["account"]["email"] = json!("User@EXAMPLE.TEST");
        write_config(&sandbox, &config);
        let file = sandbox.path().join("payload.json");
        let bytes = vec![b'x'; 24_577];
        fs::write(&file, &bytes).unwrap();
        let server =
            Server::start(|request, base| match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/api/assets/shared") => Response::json(ticket(base)),
                ("PUT", "/signed-put") | ("POST", "/api/assets/shared/nested/file") => {
                    Response::json(json!({}))
                }
                _ => Response::missing(),
            });
        let output = command(
            &sandbox,
            &server,
            &["share", "put", file.to_str().unwrap(), "--acl", acl, "--code", "old-code"],
        )
        .env("HCLI_PORTAL_URL", "https://portal.example.test")
        .output()
        .unwrap();
        assert_success(&output);
        let requests = server.requests();
        assert_eq!(requests.len(), 3);
        let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(body["filename"], "payload.json");
        assert_eq!(body["size"], bytes.len());
        assert_eq!(body["checksum"], format!("{:x}", Sha256::digest(&bytes)));
        assert_eq!(body["status"], "active");
        assert_eq!(body["force"], false);
        assert_eq!(body["code"], "old-code");
        assert_eq!(body["metadata"], json!({"acl_type": acl}));
        assert_eq!(
            body["allowed_segments"],
            if acl == "domain" {
                json!(["authenticated", "@example.test"])
            } else {
                json!(["authenticated"])
            }
        );
        if acl == "private" {
            assert_eq!(body["allowed_emails"], json!(["User@EXAMPLE.TEST"]));
        } else {
            assert!(body.get("allowed_emails").is_none());
        }
        assert!(body.get("allowed_editions").is_none());
        assert!(requests[0].headers.contains("fixture-key"));
        assert!(!requests[1].headers.to_lowercase().contains("authorization:"));
        assert!(!requests[1].headers.contains("fixture-key"));
        assert!(requests[1].headers.contains("application/json"));
        assert_eq!(requests[1].body, bytes);
        assert_eq!(requests[2].body, b"{}");
        let output = String::from_utf8_lossy(&output.stdout);
        assert!(output.starts_with("✓ File uploaded successfully!\nShare Code: fixture-code\n"));
        assert!(output.contains("https://portal.example.test/share/fixture-code"), "{output}");
        assert!(output.contains(&format!("{}/api/assets/s/fixture-code", server.url)));
        assert!(!output.contains("invalid.test"));
        let saved: Value =
            serde_json::from_slice(&fs::read(sandbox.config_path()).unwrap()).unwrap();
        assert_ne!(
            saved["hcli.credentials"]["credentials"]["account"]["last_used"],
            config["hcli.credentials"]["credentials"]["account"]["last_used"]
        );
    }
}

#[test]
fn asset_metadata_and_permission_lists_preserve_empty_values_and_whitespace() {
    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("key", "fixture-key"));
    let file = sandbox.path().join("payload.zip");
    fs::write(&file, b"fixture bytes").unwrap();
    let server = Server::start(|request, base| match request.path.as_str() {
        "/api/assets/buckets/fixture" => Response::json(bucket()),
        "/api/assets/fixture" => {
            let mut response = ticket(base);
            response["url"] = json!("");
            Response::json(response)
        }
        _ => Response::missing(),
    });
    assert_success(
        &command(
            &sandbox,
            &server,
            &[
                "asset",
                "put",
                file.to_str().unwrap(),
                "-b",
                "fixture",
                "-m",
                "version=old",
                "-m",
                "version=",
                "-m",
                " spaced =a=b",
                "--allowed-segments",
                "alpha, beta,,",
                "--allowed-emails",
                "",
                "--allowed-editions",
                ",any_edition, ",
                "--force",
            ],
        )
        .output()
        .unwrap(),
    );
    let requests = server.requests();
    assert_eq!(requests.len(), 2, "empty PUT URL must skip transfer and confirmation");
    let body: Value = serde_json::from_slice(&requests[1].body).unwrap();
    assert_eq!(body["metadata"], json!({"version": "", " spaced ": "a=b"}));
    assert_eq!(body["allowed_segments"], json!(["alpha", " beta", "", ""]));
    assert_eq!(body["allowed_editions"], json!(["", "any_edition", " "]));
    assert!(body.get("allowed_emails").is_none());
    assert!(body.get("code").is_none());
    assert_eq!(body["force"], true);
}

#[test]
fn environment_keys_retain_upstreams_async_user_placeholder_for_permissions() {
    for acl in ["private", "domain"] {
        let sandbox = Sandbox::new();
        let file = sandbox.path().join("payload");
        fs::write(&file, b"fixture").unwrap();
        let server = Server::start(|_, base| {
            let mut response = ticket(base);
            response["url"] = Value::Null;
            Response::json(response)
        });
        assert_success(
            &command(
                &sandbox,
                &server,
                &["share", "put", file.to_str().unwrap(), "--acl", acl, "--code", ""],
            )
            .env("HCLI_API_KEY", "fixture-env-key")
            .output()
            .unwrap(),
        );
        let requests = server.requests();
        assert_eq!(requests.len(), 1);
        let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert!(body.get("code").is_none());
        if acl == "domain" {
            assert_eq!(body["allowed_segments"], json!(["authenticated", "@"]));
        } else {
            assert_eq!(body["allowed_emails"], json!(["api-key-user"]));
        }
    }
}

#[test]
fn missing_or_malformed_bucket_schemas_fail_before_upload() {
    for failure in ["missing", "filename", "metadata", "requiredMetadata", "description", "example"]
    {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let file = sandbox.path().join("payload");
        fs::write(&file, b"fixture").unwrap();
        let server = Server::start(move |_, _| {
            if failure == "missing" {
                return Response::missing();
            }
            let mut response = bucket();
            if ["description", "example"].contains(&failure) {
                response["requiredMetadata"]["version"].as_object_mut().unwrap().remove(failure);
            } else {
                response.as_object_mut().unwrap().remove(failure);
            }
            Response::json(response)
        });
        let output = command(
            &sandbox,
            &server,
            &["asset", "put", file.to_str().unwrap(), "-b", "fixture", "-m", "version=9.4"],
        )
        .output()
        .unwrap();
        assert!(!output.status.success(), "{failure}");
        let requests = server.requests();
        assert_eq!(requests.len(), 1, "{failure}");
        assert_eq!(requests[0].method, "GET");
    }
}

#[test]
fn invalid_inputs_and_bucket_metadata_fail_without_starting_an_upload() {
    for metadata in [None, Some("missing-equals"), Some("other=value")] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let file = sandbox.path().join("payload");
        fs::write(&file, b"fixture").unwrap();
        let server = Server::start(|request, _| match request.path.as_str() {
            "/api/assets/buckets/fixture" => Response::json(bucket()),
            _ => Response::missing(),
        });
        let mut args = vec!["asset", "put", file.to_str().unwrap(), "-b", "fixture"];
        if let Some(metadata) = metadata {
            args.extend(["-m", metadata]);
        }
        let output = command(&sandbox, &server, &args).output().unwrap();
        assert!(!output.status.success());
        assert_eq!(server.requests().len(), usize::from(metadata.is_some()));
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        if metadata == Some("missing-equals") {
            assert!(diagnostic.contains("KEY=VALUE"), "{diagnostic}");
        } else if metadata == Some("other=value") {
            assert!(
                diagnostic.contains("Missing required metadata fields: version"),
                "{diagnostic}"
            );
        }
        assert!(server.requests().iter().all(|request| request.method == "GET"));
    }
    let sandbox = Sandbox::new();
    let server = Server::start(|_, _| Response::missing());
    for options in [vec!["--acl", "invalid"], vec!["--acl", "private", "--code", "code", "--force"]]
    {
        let mut args = vec!["share", "put", "missing"];
        args.extend(options);
        assert!(!command(&sandbox, &server, &args).output().unwrap().status.success());
    }
    assert!(server.requests().is_empty());
}

#[test]
fn malformed_tickets_and_failed_transfers_never_confirm_or_report_success() {
    for failure in ["key", "code", "version", "url", "put", "confirm"] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let file = sandbox.path().join("payload");
        fs::write(&file, b"fixture").unwrap();
        let server = Server::start(move |request, base| match request.path.as_str() {
            "/api/assets/shared" => {
                let mut response = ticket(base);
                if ["key", "code", "version"].contains(&failure) {
                    response.as_object_mut().unwrap().remove(failure);
                } else if failure == "url" {
                    response["url"] = json!(42);
                }
                Response::json(response)
            }
            "/signed-put" => Response {
                status: if failure == "put" {
                    500
                } else {
                    200
                },
                ..Response::json(json!({}))
            },
            "/api/assets/shared/nested/file" => Response {
                status: if failure == "confirm" {
                    500
                } else {
                    200
                },
                ..Response::json(json!({}))
            },
            _ => Response::missing(),
        });
        let output =
            command(&sandbox, &server, &["share", "put", file.to_str().unwrap(), "-a", "private"])
                .output()
                .unwrap();
        assert!(!output.status.success(), "{failure}");
        assert!(
            !String::from_utf8_lossy(&output.stdout).contains("uploaded successfully"),
            "{failure}"
        );
        assert_eq!(
            server.requests().len(),
            match failure {
                "key" | "put" => 2,
                "code" | "version" | "confirm" => 3,
                _ => 1,
            },
            "{failure}"
        );
    }
}

#[cfg(unix)]
#[test]
fn share_visibility_prompt_defaults_to_authenticated_and_cancels_without_upload() {
    use support::terminal::Terminal;
    for cancel in [false, true] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let file = sandbox.path().join("payload");
        fs::write(&file, b"fixture").unwrap();
        let server = Server::start(|request, base| match request.path.as_str() {
            "/api/assets/shared" => {
                let mut response = ticket(base);
                response.as_object_mut().unwrap().remove("url");
                Response::json(response)
            }
            _ => Response::missing(),
        });
        let mut terminal =
            Terminal::start(command(&sandbox, &server, &["share", "put", file.to_str().unwrap()]));
        terminal.wait_for("Pick a visibility");
        terminal.send(if cancel {
            "\x03"
        } else {
            "\r"
        });
        let (status, output) = terminal.finish();
        assert!(status.success(), "{output}");
        let requests = server.requests();
        if cancel {
            assert!(requests.is_empty());
        } else {
            assert_eq!(requests.len(), 1);
            let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
            assert_eq!(body["metadata"]["acl_type"], "authenticated");
        }
    }
}

#[cfg(unix)]
#[test]
fn visibility_selection_wraps_and_conflicts_are_checked_after_the_prompt() {
    use support::terminal::Terminal;
    for (keys, force, code, selected, status) in [
        ("j\r", false, None, Some("private"), 0),
        ("k\r", false, None, Some("domain"), 0),
        ("\x0e\r", false, None, Some("private"), 0),
        ("\x10\r", false, None, Some("domain"), 0),
        ("\x1b\r", false, None, Some("authenticated"), 0),
        ("\x11", false, None, None, 0),
        ("\r", true, Some("old-code"), None, 1),
        ("\x03", true, Some("old-code"), None, 0),
        ("\r", true, Some(""), Some("authenticated"), 0),
    ] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let file = sandbox.path().join("payload");
        fs::write(&file, b"fixture").unwrap();
        let server = Server::start(|_, base| {
            let mut response = ticket(base);
            response["url"] = Value::Null;
            Response::json(response)
        });
        let mut args = vec!["share", "put", file.to_str().unwrap()];
        if force {
            args.push("--force");
        }
        if let Some(code) = code {
            args.extend(["--code", code]);
        }
        let mut terminal = Terminal::start(command(&sandbox, &server, &args));
        terminal.wait_for("Pick a visibility");
        terminal.send(keys);
        let (actual, output) = terminal.finish();
        assert_eq!(actual.code(), Some(status), "{keys:?}: {output}");
        let requests = server.requests();
        assert_eq!(requests.len(), usize::from(selected.is_some()), "{keys:?}");
        if let Some(acl) = selected {
            let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
            assert_eq!(body["metadata"]["acl_type"], acl);
            assert_eq!(body["force"], force);
            assert!(body.get("code").is_none());
        }
    }
}
