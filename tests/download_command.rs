//! Download selection and tag contracts against a local API.

mod support;

use serde_json::{Value, json};
use std::fs;
use support::{
    auth::*,
    http::{Response, Server},
    *,
};

fn asset(key: &str, base: &str) -> Value {
    json!({"key": key, "filename": "API-name", "url": format!("{base}/payload.bin")})
}

fn tag(name: &str, key: &str) -> Value {
    json!({"tag": name, "key": key, "description": "Fixture", "bucket": "installers",
        "category": "ida-pro", "channel": "release", "version": "9.4"})
}

#[test]
fn direct_patterns_ignore_case_and_skip_failed_assets() {
    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("key", "fixture-key"));
    let server = Server::start(|request, base| match request.path.as_str() {
        "/api/assets/installers?type=file&limit=1000&offset=0" => Response::json(json!({
        "offset": 0, "limit": 1000, "total": 3, "items": [
            asset("release/FAILED.ZIP", base), asset("/release/VALID.ZIP", base), asset("release/unrelated.run", base)
        ]})),
        "/api/assets/installers/release/VALID.ZIP" => {
            Response::json(asset("/release/VALID.ZIP", base))
        }
        "/payload.bin" => Response {
            status: 200,
            content_type: "application/octet-stream",
            body: b"fixture".to_vec(),
        },
        _ => Response::missing(),
    });
    let output = command(
        &sandbox,
        &server,
        &["download", "--pattern", "\\.zip$", "--output-dir", "~/output"],
    )
    .output()
    .unwrap();
    assert_success(&output);
    assert_eq!(fs::read(sandbox.path().join("output/payload.bin")).unwrap(), b"fixture");
    assert_eq!(
        fs::read(sandbox.path().join("cache/downloads/release/VALID.ZIP")).unwrap(),
        b"fixture"
    );
    let paths: Vec<_> = server.requests().into_iter().map(|request| request.path).collect();
    assert_eq!(
        paths,
        [
            "/api/assets/installers?type=file&limit=1000&offset=0",
            "/api/assets/installers/release/FAILED.ZIP",
            "/api/assets/installers/release/VALID.ZIP",
            "/payload.bin"
        ]
    );
}

#[test]
fn pattern_errors_and_empty_matches_are_successful_but_missing_direct_patterns_fail() {
    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("key", "fixture-key"));
    let server = Server::start(|_, _| {
        Response::json(json!({"offset": 0, "limit": 1000, "total": 0, "items": []}))
    });
    for pattern in ["[", "not-found"] {
        assert_success(
            &command(&sandbox, &server, &["download", "--pattern", pattern]).output().unwrap(),
        );
    }
    let count = server.requests().len();
    for args in [
        vec!["download", "--mode", "direct"],
        vec!["download", "--mode", "direct", "--pattern", ""],
    ] {
        assert!(!command(&sandbox, &server, &args).output().unwrap().status.success());
    }
    assert_eq!(server.requests().len(), count);
}

#[test]
fn python_style_lookarounds_and_backreferences_select_matching_assets() {
    for pattern in [
        r"(?<=release/)repeat(?=-repeat\.zip$)",
        r"/(\w+)-\1\.zip$",
        r"/(?P<word>\w+)-(?P=word)\.zip$",
    ] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let server = Server::start(|request, base| match request.path.as_str() {
            "/api/assets/installers?type=file&limit=1000&offset=0" => Response::json(json!({
                "offset": 0, "limit": 1000, "total": 2,
                "items": [asset("release/repeat-repeat.zip", base), asset("release/repeat-other.zip", base)]
            })),
            "/api/assets/installers/release/repeat-repeat.zip" => {
                Response::json(asset("release/repeat-repeat.zip", base))
            }
            "/payload.bin" => Response {
                status: 200,
                content_type: "application/octet-stream",
                body: b"fixture".to_vec(),
            },
            _ => Response::missing(),
        });
        assert_success(
            &command(
                &sandbox,
                &server,
                &[
                    "download",
                    "--pattern",
                    pattern,
                    "--output-dir",
                    sandbox.path().to_str().unwrap(),
                ],
            )
            .output()
            .unwrap(),
        );
        let requests = server.requests();
        assert_eq!(requests.len(), 3, "{pattern}");
        assert_eq!(requests[1].path, "/api/assets/installers/release/repeat-repeat.zip");
    }
}

#[test]
fn tag_lookup_accepts_wrapped_and_direct_lists_and_prefers_exact_case() {
    for wrapped in [false, true] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let server = Server::start(move |request, base| match request.path.as_str() {
            "/api/assets/tags" => {
                let tags = json!([
                    tag("IDA-PRO:latest:test", "wrong-key"),
                    tag("ida-pro:latest:test", "exact-key")
                ]);
                Response::json(if wrapped {
                    json!({"tags": tags})
                } else {
                    tags
                })
            }
            "/api/assets/installers/exact-key" => Response::json(asset("exact-key", base)),
            "/payload.bin" => Response {
                status: 200,
                content_type: "application/octet-stream",
                body: b"fixture".to_vec(),
            },
            _ => Response::missing(),
        });
        assert_success(
            &command(
                &sandbox,
                &server,
                &[
                    "download",
                    "ida-pro:latest:test",
                    "--output-dir",
                    sandbox.path().to_str().unwrap(),
                ],
            )
            .output()
            .unwrap(),
        );
        assert_eq!(server.requests()[1].path, "/api/assets/installers/exact-key");
    }
}

#[test]
fn unresolved_tags_return_success_with_suggestions_but_failed_tag_lists_fail() {
    for fails in [false, true] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let server = Server::start(move |_, _| {
            if fails {
                Response::missing()
            } else {
                Response::json(json!({"tags": [tag("other:latest:test", "unused")]}))
            }
        });
        let output =
            command(&sandbox, &server, &["download", "missing:latest:test"]).output().unwrap();
        assert_eq!(output.status.success(), !fails);
        assert_eq!(server.requests().len(), 2);
        if !fails {
            assert!(String::from_utf8_lossy(&output.stderr).contains("other:latest:test"));
        }
    }
}

#[test]
fn selected_assets_without_successful_content_return_failure() {
    for descriptor in [
        json!({}),
        json!({"key": "fixture", "filename": "file"}),
        json!({"key": "fixture", "filename": "file", "url": ""}),
    ] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let server = Server::start(move |_, _| Response::json(descriptor.clone()));
        assert!(
            !command(&sandbox, &server, &["download", "fixture"])
                .output()
                .unwrap()
                .status
                .success()
        );
        assert_eq!(server.requests().len(), 1);
    }
}

#[cfg(unix)]
#[test]
fn navigation_preserves_api_folder_order_and_can_return_to_the_parent() {
    use support::terminal::Terminal;
    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("key", "fixture-key"));
    let server = Server::start(|request, base| match request.path.as_str() {
        "/api/assets/installers?type=file&view=tree&limit=1000&offset=0" => Response::json(json!([
            {"name": "9.1", "type": "folder", "asset": asset("", base), "children": [
                {"name": "inside", "type": "file", "asset": asset("first", base)}
            ]},
            {"name": "9.4", "type": "folder", "asset": asset("", base), "children": [
                {"name": "inside", "type": "file", "asset": asset("second", base)}
            ]}
        ])),
        "/api/assets/installers/first" => Response::json(asset("first", base)),
        "/payload.bin" => Response {
            status: 200,
            content_type: "application/octet-stream",
            body: b"fixture".to_vec(),
        },
        _ => Response::missing(),
    });
    let mut terminal = Terminal::start(command(
        &sandbox,
        &server,
        &["download", "--output-dir", sandbox.path().to_str().unwrap()],
    ));
    terminal.wait_for("9.1/");
    terminal.send("\r");
    terminal.wait_for("/9.1");
    terminal.send("\r");
    terminal.wait_for("9.1/");
    terminal.send("\r");
    terminal.wait_for("/9.1");
    terminal.send("\x1b[B\r");
    let (status, output) = terminal.finish();
    assert!(status.success(), "{output}");
    assert_eq!(server.requests()[1].path, "/api/assets/installers/first");
}
