//! Server validation, command constraints, and terminal-driven OTP authentication.
#![cfg(unix)]

mod support;

use std::fs;

use serde_json::json;
use support::{
    http::{Response, Server},
    *,
};

use support::auth::*;

#[test]
fn stored_interactive_tokens_require_server_validation_before_api_use() {
    for response in [
        json!({"email": EMAIL}),
        json!({}),
        json!({"email": null}),
        json!({"email": 1}),
        json!({"email": ""}),
    ] {
        let valid = response["email"] == EMAIL;
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("interactive", "opaque-fixture-token"));
        let original = fs::read(sandbox.config_path()).unwrap();
        let server = Server::start(move |request, _| match request.path.as_str() {
            "/auth/v1/user" => {
                assert_eq!(request.method, "GET");
                assert!(request.headers.to_lowercase().contains("apikey: fixture-anon"));
                assert!(request.headers.contains("Bearer opaque-fixture-token"));
                Response::json(response.clone())
            }
            "/api/keys" => {
                assert!(valid);
                assert!(request.headers.contains("Bearer opaque-fixture-token"));
                Response::json(json!([]))
            }
            _ => Response::missing(),
        });
        let output = command(&sandbox, &server, &["auth", "key", "list"]).output().unwrap();
        assert_eq!(output.status.success(), valid, "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(
            server.requests().len(),
            if valid {
                2
            } else {
                1
            }
        );
        assert_eq!(fs::read(sandbox.config_path()).unwrap(), original);
    }
}

#[test]
fn revoked_tokens_are_anonymous_for_optional_repositories_and_logged_out_in_whoami() {
    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("interactive", "revoked-fixture-token"));
    let server = Server::start(|request, _| match request.path.as_str() {
        "/auth/v1/user" => Response {
            status: 401,
            ..Response::json(json!({"error": "revoked"}))
        },
        "/repository.json" => {
            assert!(!request.headers.to_lowercase().contains("authorization:"));
            Response::json(json!({"version": 1, "plugins": []}))
        }
        _ => Response::missing(),
    });
    let whoami = command(&sandbox, &server, &["whoami"]).output().unwrap();
    assert_success(&whoami);
    assert!(String::from_utf8_lossy(&whoami.stderr).contains("not logged in"));
    let repository = format!("{}/repository.json", server.url);
    let search = command(&sandbox, &server, &["plugin", "--repo", &repository, "search", "--json"])
        .output()
        .unwrap();
    assert_success(&search);
    assert!(server.requests().iter().any(|request| request.path == "/repository.json"));
}

#[test]
fn auth_type_and_credential_constraints_fail_before_command_side_effects() {
    for (kind, required, allowed) in [
        ("key", "key", true),
        ("interactive", "interactive", true),
        ("key", "interactive", false),
        ("interactive", "key", false),
        ("key", "invalid", false),
        ("key", "KEY", false),
    ] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored(kind, "fixture-token"));
        let server = Server::start(|request, _| match request.path.as_str() {
            "/auth/v1/user" => Response::json(json!({"email": EMAIL})),
            "/api/keys" => Response::json(json!([])),
            _ => Response::missing(),
        });
        let output = command(&sandbox, &server, &["--auth", required, "auth", "key", "list"])
            .output()
            .unwrap();
        assert_eq!(output.status.success(), allowed, "{kind}/{required}");
        if !allowed {
            assert!(server.requests().is_empty());
        }
    }
    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("key", "fixture-key"));
    let server = Server::start(|_, _| Response::json(json!([])));
    let missing =
        command(&sandbox, &server, &["--auth-credentials", "missing", "auth", "key", "list"])
            .output()
            .unwrap();
    assert!(!missing.status.success());
    assert!(server.requests().is_empty());
    assert_success(&sandbox.run(&["--auth", "invalid", "auth", "list"]));
}

#[test]
fn environment_key_remains_usable_without_forced_managed_credentials() {
    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("interactive", "must-not-validate"));
    let server = Server::start(|request, _| {
        assert_eq!(request.path, "/api/keys");
        assert!(request.headers.to_lowercase().contains("x-api-key: environment-fixture"));
        Response::json(json!([]))
    });
    let output = command(&sandbox, &server, &["--auth", "key", "auth", "key", "list"])
        .env("HCLI_API_KEY", "environment-fixture")
        .output()
        .unwrap();
    assert_success(&output);
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn empty_tokens_do_not_establish_authentication_or_mask_stored_keys() {
    for kind in ["key", "interactive"] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored(kind, ""));
        let server = Server::start(|_, _| panic!("empty credential reached the network"));
        let output = command(&sandbox, &server, &["auth", "key", "list"]).output().unwrap();
        assert!(!output.status.success());
        assert!(server.requests().is_empty());
    }
    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("key", "stored-key"));
    let server = Server::start(|request, _| {
        assert!(request.headers.to_lowercase().contains("x-api-key: stored-key"));
        Response::json(json!([]))
    });
    let output = command(&sandbox, &server, &["auth", "key", "list"])
        .env("HCLI_API_KEY", "")
        .output()
        .unwrap();
    assert_success(&output);
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn constrained_commands_check_auth_before_files_prompts_or_api_requests() {
    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("key", "fixture-key"));
    let server = Server::start(|_, _| panic!("auth mismatch reached the API"));
    for tail in [
        vec!["download"],
        vec!["share", "put", "missing-file"],
        vec!["share", "get", "fixture-code"],
        vec!["share", "list"],
        vec!["share", "delete", "fixture-code"],
        vec!["license", "list"],
        vec!["license", "get"],
        vec!["asset", "put", "missing-file", "--bucket", "fixture", "-m", "fixture=value"],
        vec!["asset", "delete", "fixture-key", "--bucket", "fixture"],
    ] {
        let mut args = vec!["--auth", "interactive"];
        args.extend(tail);
        let output = command(&sandbox, &server, &args).output().unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("authentication type mismatch"),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert!(server.requests().is_empty());
}
