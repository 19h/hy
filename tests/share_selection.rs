//! Filtered selection, terminal cancellation and signed pagination.

mod support;

use serde_json::json;
use support::{
    auth::*,
    http::{Response, Server},
    *,
};

fn listing() -> Server {
    Server::start(|request, _| {
        if request.method == "DELETE" {
            return Response::json(json!({}));
        }
        if request.path.starts_with("/api/assets/shared?type=file&") {
            let items: Vec<_> = ["alpha", "beta", "jacket"]
                .iter()
                .map(|name| {
                    json!({
                        "filename": format!("{name}.txt"), "key": format!("/{name}.txt"),
                        "code": name, "version": 3, "size": 7,
                    })
                })
                .collect();
            return Response::json(json!({"items": items, "limit": 100, "offset": 0, "total": 3}));
        }
        Response::missing()
    })
}

#[cfg(unix)]
#[test]
fn filtered_selection_keeps_identity_and_original_submission_order() {
    use support::terminal::Terminal;
    for (keys, expected) in [
        ("beta \x7f\x7f\x7f\x7falpha \r", vec!["alpha", "beta"]),
        ("beta\x01\r", vec!["alpha", "beta", "jacket"]),
        ("beta \t\r", vec!["alpha", "jacket"]),
        ("zzzz \r", vec!["alpha"]),
        ("j \r", vec!["jacket"]),
        ("\x0e \r", vec!["beta"]),
        ("\x10 \r", vec!["jacket"]),
    ] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let server = listing();
        let mut terminal = Terminal::start(command(&sandbox, &server, &["share", "list"]));
        terminal.wait_for("Select files to manage:");
        terminal.send(keys);
        terminal.wait_for("What would you like to do?");
        terminal.send("\r");
        terminal.wait_for("Are you sure");
        terminal.send("y\r");
        let (status, output) = terminal.finish();
        assert!(status.success(), "{keys:?}: {output}");
        let deleted: Vec<_> = server
            .requests()
            .into_iter()
            .filter(|request| request.method == "DELETE")
            .map(|request| request.path)
            .collect();
        assert_eq!(
            deleted,
            expected
                .iter()
                .map(|name| format!("/api/assets/shared/{name}.txt"))
                .collect::<Vec<_>>(),
            "{keys:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn cancelling_any_management_prompt_never_starts_an_action() {
    use support::terminal::Terminal;
    for stage in ["files", "files-ctrl-q", "action", "action-ctrl-q", "directory", "empty"] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let server = listing();
        let mut process = command(&sandbox, &server, &["share", "list"]);
        process.current_dir(sandbox.path());
        let mut terminal = Terminal::start(process);
        terminal.wait_for("Select files to manage:");
        match stage {
            "files" => terminal.send("\x03"),
            "files-ctrl-q" => terminal.send("\x11"),
            "empty" => terminal.send("beta\x01\x01\r"),
            _ => {
                terminal.send(" \r");
                terminal.wait_for("What would you like to do?");
                if stage.starts_with("action") {
                    terminal.send(if stage == "action" {
                        "\x03"
                    } else {
                        "\x11"
                    });
                } else {
                    terminal.send("\x1b[B\r");
                    terminal.wait_for("Output directory");
                    terminal.send("\x03");
                }
            }
        }
        let (status, output) = terminal.finish();
        assert!(status.success(), "{stage}: {output}");
        assert!(
            output.contains(if stage == "empty" {
                "No files selected"
            } else {
                "Operation cancelled"
            }),
            "{stage}: {output}"
        );
        assert_eq!(server.requests().len(), 1, "{stage}");
        for name in ["alpha", "beta", "jacket"] {
            assert!(!sandbox.path().join(format!("{name}.txt")).exists());
        }
    }
}

#[test]
fn signed_pagination_values_are_forwarded_without_range_clamping() {
    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("key", "fixture-key"));
    let server = listing();
    let output = command(
        &sandbox,
        &server,
        &["share", "list", "--limit", "-5", "--offset", "-2", "--no-interactive"],
    )
    .output()
    .unwrap();
    assert_success(&output);
    assert_eq!(server.requests()[0].path, "/api/assets/shared?type=file&limit=-5&offset=-2");
}

#[cfg(unix)]
#[test]
fn equal_asset_values_share_selection_and_preserve_inversion_multiplicity() {
    use support::terminal::Terminal;

    for (keys, count) in [(" \r", 2), ("\t \r", 3)] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let server = Server::start(|request, _| {
            if request.method == "DELETE" {
                return Response::json(json!({}));
            }
            let items: Vec<_> = [json!(true), json!(1.0), json!(2)]
                .into_iter()
                .map(|value| {
                    json!({
                        "filename": "same.txt", "key": "/same.txt", "code": "same",
                        "size": "7.0", "version": "184467440737095516160",
                        "metadata": {"value": value},
                    })
                })
                .collect();
            Response::json(json!({"items": items, "limit": true, "offset": "0.0", "total": 3.0}))
        });
        let table =
            command(&sandbox, &server, &["share", "list", "--no-interactive"]).output().unwrap();
        assert_success(&table);
        let table = String::from_utf8_lossy(&table.stderr);
        assert!(table.contains("184467440737095516160"), "{table}");
        let mut terminal = Terminal::start(command(&sandbox, &server, &["share", "list"]));
        terminal.wait_for("Select files to manage:");
        terminal.send(keys);
        terminal.wait_for("What would you like to do?");
        terminal.send("\r");
        terminal.wait_for("Are you sure");
        terminal.send("y\r");
        let (status, output) = terminal.finish();
        assert!(status.success(), "{keys:?}: {output}");
        let deleted: Vec<_> = server
            .requests()
            .into_iter()
            .filter(|request| request.method == "DELETE")
            .map(|request| request.path)
            .collect();
        assert_eq!(deleted, vec!["/api/assets/shared/same.txt"; count], "{keys:?}");
    }
}

#[test]
fn explicit_null_asset_and_paging_integers_fail_before_management() {
    for field in ["size", "version", "offset", "limit", "total"] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let server = Server::start(move |_, _| {
            let mut response = json!({
                "items": [{"filename": "fixture", "key": "fixture"}],
                "offset": 0, "limit": 100, "total": 1,
            });
            if matches!(field, "size" | "version") {
                response["items"][0][field] = json!(null);
            } else {
                response[field] = json!(null);
            }
            Response::json(response)
        });
        let output =
            command(&sandbox, &server, &["share", "list", "--no-interactive"]).output().unwrap();
        assert!(!output.status.success(), "{field}");
        assert_eq!(server.requests().len(), 1, "{field}");
    }
}
