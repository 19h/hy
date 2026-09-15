//! Shared file lookup, output naming, overwrite decisions, and deletion contracts.

#[path = "share_operations/reference.rs"]
mod reference;
mod support;

use serde_json::{Value, json};
use std::fs;
use support::{
    auth::*,
    http::{Response, Server},
    *,
};

fn descriptor(base: &str) -> Value {
    json!({"filename": "original.txt", "key": "///nested/original.txt",
        "code": "fixture-code", "version": 3, "size": 7,
        "url": format!("{base}/signed-opaque-name"),
        "metadata": {"acl_type": "private"}})
}

fn server() -> Server {
    Server::start(|request, base| match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/api/assets/s/fixture-code?version=-1") => Response::json(descriptor(base)),
        ("GET", "/signed-opaque-name") => Response {
            status: 200,
            content_type: "application/octet-stream",
            body: b"fixture".to_vec(),
        },
        ("DELETE", "/api/assets/shared/nested/original.txt")
        | ("DELETE", "/api/assets/fixture/nested/original.txt") => Response::json(json!({})),
        ("GET", "/api/assets/shared?type=file&limit=7&offset=2") => Response::json(
            json!({"offset": 2, "limit": 7, "total": 3, "items": [descriptor(base)]}),
        ),
        _ => Response::missing(),
    })
}

#[test]
fn downloads_use_descriptor_names_and_explicit_output_paths() {
    for mode in ["default", "directory", "file", "home"] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let server = server();
        let dir = sandbox.path().join("output");
        fs::create_dir_all(&dir).unwrap();
        let target = dir.join("renamed.txt");
        let mut args = vec!["share", "get", "fixture-code"];
        match mode {
            "directory" => args.extend(["-o", dir.to_str().unwrap()]),
            "file" => args.extend(["-O", target.to_str().unwrap()]),
            "home" => args.extend(["-O", "~/output/renamed.txt"]),
            _ => {}
        }
        let output = command(&sandbox, &server, &args).current_dir(&dir).output().unwrap();
        assert_success(&output);
        let expected = if ["file", "home"].contains(&mode) {
            target
        } else {
            dir.join("original.txt")
        };
        assert_eq!(fs::read(&expected).unwrap(), b"fixture");
        reference::compare(
            "get",
            descriptor("http://fixture.invalid"),
            &expected.canonicalize().unwrap(),
            &output,
            2,
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            format!(
                "✓ File downloaded successfully!\nFile: original.txt\nSize: 7.0 B\nSaved to: {}\n",
                expected.canonicalize().unwrap().display()
            )
        );
        assert!(!dir.join("signed-opaque-name").exists());
        let requests = server.requests();
        assert_eq!(requests.len(), 2);
        assert!(requests[1].headers.contains("fixture-key"));
    }
}

#[cfg(unix)]
#[test]
fn overwrite_confirmation_preserves_or_replaces_the_existing_file() {
    use support::terminal::Terminal;
    for choice in ["cancel", "confirm", "force"] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let server = server();
        let target = sandbox.path().join("original.txt");
        fs::write(&target, b"original bytes").unwrap();
        let mut args = vec!["share", "get", "fixture-code", "-O", target.to_str().unwrap()];
        if choice == "force" {
            args.push("--force");
        }
        let mut terminal = Terminal::start(command(&sandbox, &server, &args));
        if choice != "force" {
            terminal.wait_for("Overwrite existing file?");
            terminal.send(if choice == "confirm" {
                "y\r"
            } else {
                "n\r"
            });
        }
        let (status, output) = terminal.finish();
        assert!(status.success(), "{output}");
        let expected: &[u8] = if choice == "cancel" {
            b"original bytes"
        } else {
            b"fixture"
        };
        assert_eq!(fs::read(&target).unwrap(), expected);
        assert_eq!(
            server.requests().len(),
            if choice == "cancel" {
                1
            } else {
                2
            }
        );
    }
}

#[test]
fn share_and_bucket_deletes_strip_leading_key_slashes() {
    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("key", "fixture-key"));
    let server = server();
    let output = command(&sandbox, &server, &["share", "delete", "fixture-code", "--force"])
        .output()
        .unwrap();
    assert_success(&output);
    reference::compare(
        "delete",
        descriptor("http://fixture.invalid"),
        &sandbox.path().join("unused"),
        &output,
        2,
    );
    assert_success(
        &command(
            &sandbox,
            &server,
            &["asset", "delete", "///nested/original.txt", "-b", "fixture", "--yes"],
        )
        .output()
        .unwrap(),
    );
    let paths: Vec<_> = server.requests().into_iter().map(|request| request.path).collect();
    assert_eq!(
        paths,
        [
            "/api/assets/s/fixture-code?version=-1",
            "/api/assets/shared/nested/original.txt",
            "/api/assets/fixture/nested/original.txt"
        ]
    );
}

#[test]
fn lookup_http_errors_and_malformed_descriptors_propagate_as_failures() {
    reference::verify_lookup_errors();
    for status in [401, 403, 404, 429, 500, 200] {
        for operation in ["get", "delete"] {
            let sandbox = Sandbox::new();
            write_config(&sandbox, &stored("key", "fixture-key"));
            let server = Server::start(move |_, _| Response {
                status,
                ..Response::json(json!({"key": "missing-filename"}))
            });
            let output =
                command(&sandbox, &server, &["share", operation, "fixture-code"]).output().unwrap();
            assert_eq!(output.status.code(), Some(1));
            let report = String::from_utf8_lossy(&output.stdout);
            assert!(
                report.contains(if operation == "get" {
                    "Error downloading file:"
                } else {
                    "Error during deletion:"
                }),
                "{report}"
            );
            assert!(!report.contains("File with shortcode"), "{report}");
            assert_eq!(server.requests().len(), 1);
        }
    }
}

#[test]
fn invalid_credentials_do_not_become_missing_shares() {
    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("key", "invalid\r\nkey"));
    let server = server();
    for operation in ["get", "delete"] {
        let output =
            command(&sandbox, &server, &["share", operation, "fixture-code"]).output().unwrap();
        assert!(!output.status.success());
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(diagnostic.contains("invalid API key header"), "{diagnostic}");
        assert!(!diagnostic.contains("not found"), "{diagnostic}");
    }
    assert!(server.requests().is_empty());
}

#[test]
fn noninteractive_list_displays_acl_and_sends_pagination() {
    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("key", "fixture-key"));
    let server = server();
    let output = command(
        &sandbox,
        &server,
        &["share", "list", "--limit", "7", "--offset", "2", "--interactive", "--no-interactive"],
    )
    .output()
    .unwrap();
    assert_success(&output);
    let output = String::from_utf8_lossy(&output.stderr);
    for expected in ["ACL", "private", "original.txt", "v3"] {
        assert!(output.contains(expected), "{output}");
    }
    assert_eq!(server.requests().len(), 1);
}

#[cfg(unix)]
#[test]
fn interactive_batches_continue_after_individual_failures() {
    use support::terminal::Terminal;
    for action in ["delete", "download"] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let output_dir = sandbox.path().join("downloads");
        let server = Server::start(move |request, base| {
            match (request.method.as_str(), request.path.as_str()) {
                ("GET", "/api/assets/shared?type=file&limit=100&offset=0") => {
                    let mut first = descriptor(base);
                    first["filename"] = json!("first.txt");
                    first["key"] = json!("/nested/first.txt");
                    first["code"] = json!("first");
                    let mut second = descriptor(base);
                    second["filename"] = json!("second.txt");
                    second["code"] = json!("second");
                    Response::json(
                        json!({"offset": 0, "limit": 100, "total": 2, "items": [first, second]}),
                    )
                }
                ("GET", "/api/assets/s/second?version=3") => Response::json(descriptor(base)),
                ("GET", "/signed-opaque-name") => Response {
                    status: 200,
                    content_type: "application/octet-stream",
                    body: b"fixture".to_vec(),
                },
                ("DELETE", "/api/assets/shared/nested/original.txt") => Response::json(json!({})),
                _ => Response::missing(),
            }
        });
        let mut terminal = Terminal::start(command(&sandbox, &server, &["share", "list"]));
        terminal.wait_for("Select files to manage");
        terminal.send(" \x1b[B \r");
        terminal.wait_for("What would you like to do?");
        if action == "delete" {
            terminal.send("\r");
            terminal.wait_for("Are you sure");
            terminal.send("y\r");
        } else {
            terminal.send("\x1b[B\r");
            terminal.wait_for("Output directory");
            terminal.send(&format!("\x7f\x7f{}\r", output_dir.display()));
        }
        let (status, output) = terminal.finish();
        assert!(status.success(), "{output}");
        assert!(output.contains("Failed to"), "{output}");
        let requests = server.requests();
        if action == "delete" {
            assert_eq!(requests.len(), 3);
            assert_eq!(requests[1].path, "/api/assets/shared/nested/first.txt");
            assert_eq!(requests[2].path, "/api/assets/shared/nested/original.txt");
            assert!(output.contains("Deleted: second.txt"), "{output}");
        } else {
            assert_eq!(requests.len(), 4);
            assert_eq!(requests[1].path, "/api/assets/s/first?version=3");
            assert_eq!(requests[2].path, "/api/assets/s/second?version=3");
            assert_eq!(fs::read(output_dir.join("second.txt")).unwrap(), b"fixture");
            assert!(!output_dir.join("first.txt").exists());
        }
    }
}

#[cfg(unix)]
#[test]
fn deletion_confirmation_never_sends_a_request_when_declined() {
    use support::terminal::Terminal;
    for operation in ["share", "asset"] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let server = server();
        let args = if operation == "share" {
            vec!["share", "delete", "fixture-code"]
        } else {
            vec!["asset", "delete", "nested/original.txt", "-b", "fixture"]
        };
        let mut terminal = Terminal::start(command(&sandbox, &server, &args));
        terminal.wait_for(if operation == "share" {
            "Delete file original.txt"
        } else {
            "Are you sure"
        });
        terminal.send("n\r");
        let (status, output) = terminal.finish();
        assert_eq!(status.success(), operation == "share", "{output}");
        assert!(server.requests().iter().all(|request| request.method == "GET"));
    }
}

#[test]
fn size_report_failures_preserve_downloads_and_prevent_forced_deletion() {
    for size in [json!(-1), json!("1125899906842624"), json!(null)] {
        for operation in ["get", "delete"] {
            let sandbox = Sandbox::new();
            write_config(&sandbox, &stored("key", "fixture-key"));
            let value = size.clone();
            let server = Server::start(move |request, base| {
                if request.path == "/signed-opaque-name" {
                    return Response {
                        body: b"fixture".to_vec(),
                        ..Response::json(json!({}))
                    };
                }
                let mut asset = descriptor(base);
                asset["size"] = value.clone();
                Response::json(asset)
            });
            let output =
                command(&sandbox, &server, &["share", operation, "fixture-code", "--force"])
                    .current_dir(sandbox.path())
                    .output()
                    .unwrap();
            assert_eq!(output.status.code(), Some(1), "{operation}, {size}");
            let downloaded = operation == "get" && !size.is_null();
            assert_eq!(sandbox.path().join("original.txt").exists(), downloaded);
            assert_eq!(
                server.requests().len(),
                if downloaded {
                    2
                } else {
                    1
                }
            );
            assert!(server.requests().iter().all(|request| request.method == "GET"));
            let report = String::from_utf8_lossy(&output.stdout);
            assert_eq!(report.contains("File downloaded successfully"), downloaded, "{report}");
            let mut asset = descriptor("http://fixture.invalid");
            asset["size"] = size.clone();
            reference::compare(
                operation,
                asset,
                &sandbox.path().join("original.txt"),
                &output,
                server.requests().len(),
            );
        }
    }
}

#[test]
fn missing_download_urls_report_success_without_publishing_a_file() {
    for url in [json!(null), json!("")] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let mut asset = descriptor("http://fixture.invalid");
        asset["url"] = url;
        let response = asset.clone();
        let server = Server::start(move |_, _| Response::json(response.clone()));
        let target = sandbox.path().join("original.txt");
        let output = command(&sandbox, &server, &["share", "get", "fixture-code"])
            .current_dir(sandbox.path())
            .output()
            .unwrap();
        assert_success(&output);
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "Error: No download URL available for file\n"
        );
        assert_eq!(server.requests().len(), 1);
        assert!(!target.exists());
        reference::compare("get", asset, &target, &output, 1);
    }
}

#[test]
fn conflicting_download_destinations_abort_before_lookup() {
    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("key", "fixture-key"));
    let server = server();
    let output = command(
        &sandbox,
        &server,
        &["share", "get", "fixture-code", "-o", "directory", "-O", "file"],
    )
    .output()
    .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "Error: --output-dir and --output-file cannot be used together\n"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "Aborted!\n");
    assert!(server.requests().is_empty());
}

#[test]
fn redirected_confirmations_follow_click_and_rich_input_rules() {
    use std::io::Write;
    use std::process::Stdio;

    for (operation, input, accepted, success, retried) in [
        ("get", "yes\n", true, true, false),
        ("get", "\n", false, true, false),
        ("get", "invalid\nYES\n", true, true, true),
        ("get", "", false, false, false),
        ("delete", "y\n", true, true, false),
        ("delete", "yes\nn\n", false, true, true),
        ("delete", "\nY\n", true, true, true),
        ("delete", "", false, false, false),
    ] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let server = server();
        let target = sandbox.path().join("original.txt");
        fs::write(&target, b"original bytes").unwrap();
        let mut child = command(&sandbox, &server, &["share", operation, "fixture-code"])
            .current_dir(sandbox.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.success(), success, "{operation}, {input:?}");
        assert_eq!(
            server.requests().len(),
            if accepted {
                2
            } else {
                1
            }
        );
        if operation == "get" {
            assert_eq!(
                fs::read(target).unwrap(),
                if accepted {
                    b"fixture".as_slice()
                } else {
                    b"original bytes"
                }
            );
        }
        let report = String::from_utf8_lossy(&output.stdout);
        let error = if operation == "get" {
            "Error: invalid input"
        } else {
            "Please enter Y or N"
        };
        assert_eq!(report.contains(error), retried, "{report}");
    }
}
