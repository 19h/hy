//! Upstream credential files and persistence failures through isolated CLI processes.
#![cfg(unix)]

mod support;

use std::fs;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Value, json};
use support::{
    http::{Response, Server},
    *,
};

const EMAIL: &str = "account@example.test";
const UPSTREAM_TIMESTAMP: &str = "2026-09-15T10:00:00.123456+00:00Z";

fn credential(name: &str, kind: &str, token: &str) -> Value {
    json!({
        "name": name, "type": kind, "email": EMAIL, "token": token,
        "created_at": UPSTREAM_TIMESTAMP, "last_used": UPSTREAM_TIMESTAMP,
    })
}

fn write_config(sandbox: &Sandbox, value: &Value) {
    let path = sandbox.config_path();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn read_config(sandbox: &Sandbox) -> Value {
    serde_json::from_slice(&fs::read(sandbox.config_path()).unwrap()).unwrap()
}

fn jwt(email: &str, expiry: i64) -> String {
    let payload = URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(&json!({"email": email, "exp": expiry})).unwrap());
    format!("e30.{payload}.fixture")
}

fn session(token: &str, refresh: &str) -> Value {
    json!({"access_token": token, "refresh_token": refresh, "expires_in": 3600,
        "expires_at": 4102444800_i64, "token_type": "bearer", "user": {"email": EMAIL}})
}

#[test]
fn upstream_timestamps_and_credential_insertion_order_survive_mutation() {
    let sandbox = Sandbox::new();
    let mut entries = serde_json::Map::new();
    for name in ["zeta", "alpha", "current"] {
        entries.insert(name.into(), credential(name, "key", name));
    }
    write_config(
        &sandbox,
        &json!({
            "hcli.credentials": {"default": "current", "credentials": entries},
            "unrelated": {"keep": true},
        }),
    );
    assert_success(&sandbox.run(&["auth", "list"]));
    let mut terminal =
        support::terminal::Terminal::start(sandbox.command(&["logout", "--name", "current"]));
    terminal.wait_for("Remove credentials 'current'");
    terminal.send("y\r");
    let (status, output) = terminal.finish();
    assert!(status.success(), "{output}");
    let saved = read_config(&sandbox);
    assert_eq!(saved["hcli.credentials"]["default"], "zeta");
    let entries = saved["hcli.credentials"]["credentials"].as_object().unwrap();
    assert_eq!(entries.keys().map(String::as_str).collect::<Vec<_>>(), ["zeta", "alpha"]);
    assert_eq!(entries["zeta"]["created_at"], UPSTREAM_TIMESTAMP);
    assert_eq!(entries["zeta"]["last_used"], UPSTREAM_TIMESTAMP);
    assert_eq!(saved["unrelated"]["keep"], true);
}

#[test]
fn malformed_stored_credentials_fail_without_replacing_the_file() {
    for invalid in [json!({"credentials": {"bad": {"token": "fixture"}}}), json!([])] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &json!({"hcli.credentials": invalid}));
        let original = fs::read(sandbox.config_path()).unwrap();
        let output = sandbox.run(&["auth", "default", "new"]);
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("invalid stored credentials"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(fs::read(sandbox.config_path()).unwrap(), original);
    }
}

#[test]
fn api_key_install_reports_persistence_errors_after_validation() {
    let sandbox = Sandbox::new();
    write_config(&sandbox, &json!({"unrelated": 42}));
    let original = fs::read(sandbox.config_path()).unwrap();
    let path = sandbox.config_path();
    let saved = path.with_extension("saved");
    let preserved = saved.clone();
    let server = Server::start(move |request, _| {
        assert_eq!(request.path, "/api/whoami");
        fs::rename(&path, &saved).unwrap();
        fs::create_dir(&path).unwrap();
        Response::json(json!({"email": EMAIL}))
    });
    let output = sandbox
        .command(&[
            "auth",
            "key",
            "install",
            "--key",
            "fixture-key",
            "--key-name",
            "installed",
            "--set-default",
        ])
        .env("HCLI_API_URL", &server.url)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("I/O error"));
    assert!(!error.contains("created!"));
    assert_eq!(fs::read(preserved).unwrap(), original);
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn expired_tokens_refresh_and_persist_before_the_api_request() {
    for minimal in [false, true] {
        let sandbox = Sandbox::new();
        let expired = jwt(EMAIL, 1);
        let fresh = jwt(EMAIL, 4102444800);
        write_config(
            &sandbox,
            &json!({
                "hcli.credentials": {"default": "account", "credentials": {"account": credential("account", "interactive", &expired)}},
                "supabase.auth.token": session(&expired, "old-refresh").to_string(),
            }),
        );
        let refreshed = fresh.clone();
        let server = Server::start(move |request, _| match request.path.as_str() {
            "/auth/v1/token?grant_type=refresh_token" => {
                assert_eq!(
                    serde_json::from_slice::<Value>(&request.body).unwrap()["refresh_token"],
                    "old-refresh"
                );
                Response::json(if minimal {
                    json!({"access_token": refreshed, "refresh_token": "new-refresh"})
                } else {
                    session(&refreshed, "new-refresh")
                })
            }
            "/api/keys" => {
                assert!(request.headers.contains(&format!("Bearer {refreshed}")));
                Response::json(json!([]))
            }
            "/auth/v1/user" if request.headers.contains(&format!("Bearer {refreshed}")) => {
                Response::json(json!({"email": EMAIL}))
            }
            "/auth/v1/user" => Response {
                status: 401,
                ..Response::json(json!({}))
            },
            _ => Response::missing(),
        });
        let output = sandbox
            .command(&["auth", "key", "list"])
            .env("HCLI_API_URL", &server.url)
            .env("HCLI_SUPABASE_URL", &server.url)
            .env("HCLI_SUPABASE_ANON_KEY", "fixture-anon-key")
            .output()
            .unwrap();
        assert_success(&output);
        assert_eq!(
            server.requests().iter().map(|request| request.path.as_str()).collect::<Vec<_>>(),
            [
                "/auth/v1/user",
                "/auth/v1/token?grant_type=refresh_token",
                "/auth/v1/user",
                "/api/keys"
            ]
        );
        let saved = read_config(&sandbox);
        assert_eq!(saved["hcli.credentials"]["credentials"]["account"]["token"], fresh);
        assert_eq!(
            saved["hcli.credentials"]["credentials"]["account"]["created_at"],
            UPSTREAM_TIMESTAMP
        );
        let saved_session: Value =
            serde_json::from_str(saved["supabase.auth.token"].as_str().unwrap()).unwrap();
        assert_eq!(saved_session["refresh_token"], "new-refresh");
    }
}

#[test]
fn refresh_failures_and_foreign_sessions_cannot_reach_the_api() {
    for failure in
        ["http", "different-account", "malformed-session", "write", "changed-account-response"]
    {
        let sandbox = Sandbox::new();
        let expired = jwt(EMAIL, 1);
        let stored_token = if failure == "different-account" {
            jwt("other@example.test", 1)
        } else {
            expired.clone()
        };
        let stored_session = if failure == "malformed-session" {
            "invalid-json".into()
        } else {
            session(&stored_token, "old-refresh").to_string()
        };
        write_config(
            &sandbox,
            &json!({
                "hcli.credentials": {"default": "account", "credentials": {"account": credential("account", "interactive", &expired)}},
                "supabase.auth.token": stored_session,
            }),
        );
        let original = fs::read(sandbox.config_path()).unwrap();
        let path = sandbox.config_path();
        let saved = path.with_extension("saved");
        let preserved = saved.clone();
        let server = Server::start(move |request, _| {
            if request.path == "/auth/v1/user" {
                if request.headers.contains(&format!("Bearer {expired}")) {
                    return Response {
                        status: 401,
                        ..Response::json(json!({}))
                    };
                }
                return Response::json(json!({"email": EMAIL}));
            }
            assert_eq!(request.path, "/auth/v1/token?grant_type=refresh_token");
            if failure == "write" {
                fs::rename(&path, &saved).unwrap();
                fs::create_dir(&path).unwrap();
                Response::json(session(&jwt(EMAIL, 4102444800), "new-refresh"))
            } else if failure == "changed-account-response" {
                Response::json(session(&jwt("other@example.test", 4102444800), "new-refresh"))
            } else {
                Response::missing()
            }
        });
        let output = sandbox
            .command(&["auth", "key", "list"])
            .env("HCLI_API_URL", &server.url)
            .env("HCLI_SUPABASE_URL", &server.url)
            .env("HCLI_SUPABASE_ANON_KEY", "fixture-anon-key")
            .output()
            .unwrap();
        assert!(!output.status.success(), "{failure}");
        assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
        assert_eq!(
            server.requests().len(),
            usize::from(matches!(failure, "http" | "write" | "changed-account-response"))
                + usize::from(failure == "write")
                + 1
        );
        assert_eq!(
            fs::read(if failure == "write" {
                preserved
            } else {
                sandbox.config_path()
            })
            .unwrap(),
            original
        );
    }
}

#[test]
fn invalid_key_headers_fail_without_panicking_or_sending_a_request() {
    let sandbox = Sandbox::new();
    write_config(
        &sandbox,
        &json!({
            "hcli.credentials": {"default": "bad", "credentials": {"bad": credential("bad", "key", "fixture\r\nInjected: value")}},
        }),
    );
    let server = Server::start(|_, _| Response::json(json!([])));
    let output = sandbox
        .command(&["auth", "key", "list"])
        .env("HCLI_API_URL", &server.url)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("invalid API key header"), "{error}");
    assert!(!error.contains("Injected"));
    assert!(!error.contains("panicked"));
    assert!(server.requests().is_empty());
}

#[test]
fn key_install_uses_the_canonical_name_option_and_rejects_validation_failures() {
    for valid in [false, true] {
        let sandbox = Sandbox::new();
        let server = Server::start(move |request, _| {
            assert_eq!(request.path, "/api/whoami");
            assert!(request.headers.to_ascii_lowercase().contains("x-api-key: fixture-key"));
            if valid {
                Response::json(json!({"email": EMAIL}))
            } else {
                Response::missing()
            }
        });
        let output = sandbox
            .command(&[
                "auth",
                "key",
                "install",
                "--key",
                "fixture-key",
                "--key-name",
                "canonical",
                "--name",
                "legacy",
            ])
            .env("HCLI_API_URL", &server.url)
            .output()
            .unwrap();
        assert_eq!(output.status.success(), valid);
        if valid {
            let saved = read_config(&sandbox);
            assert_eq!(saved["hcli.credentials"]["default"], "canonical");
            assert_eq!(
                saved["hcli.credentials"]["credentials"]["canonical"]["token"],
                "fixture-key"
            );
            assert!(saved["hcli.credentials"]["credentials"].get("legacy").is_none());
        } else {
            assert!(!sandbox.config_path().exists());
        }
    }
}

#[test]
fn forced_credentials_require_a_managed_source_without_changing_the_default() {
    for environment in [false, true] {
        let sandbox = Sandbox::new();
        write_config(
            &sandbox,
            &json!({
                "hcli.credentials": {"default": "first", "credentials": {
                    "first": credential("first", "key", "first-key"),
                    "second": credential("second", "key", "second-key"),
                }},
            }),
        );
        let original = fs::read(sandbox.config_path()).unwrap();
        let expected = if environment {
            "environment-key"
        } else {
            "second-key"
        };
        let server = Server::start(move |request, _| {
            assert!(
                request.headers.to_ascii_lowercase().contains(&format!("x-api-key: {expected}"))
            );
            Response::json(json!([]))
        });
        let mut command = sandbox.command(&["--auth-credentials", "second", "auth", "key", "list"]);
        command.env("HCLI_API_URL", &server.url);
        if environment {
            command.env("HCLI_API_KEY", "environment-key");
        }
        let output = command.output().unwrap();
        if environment {
            assert!(!output.status.success());
            assert!(
                String::from_utf8_lossy(&output.stderr).contains("credentials 'second' not found")
            );
        } else {
            assert_success(&output);
        }
        assert_eq!(fs::read(sandbox.config_path()).unwrap(), original);
        assert_eq!(server.requests().len(), usize::from(!environment));
    }
}
