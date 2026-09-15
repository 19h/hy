//! Terminal-driven authentication and credential removal regressions.
#![cfg(unix)]

mod support;

use serde_json::{Value, json};
use std::fs;
use support::{
    auth::*,
    http::{Response, Server},
    terminal::Terminal,
    *,
};

#[test]
fn forced_otp_login_signs_out_and_clears_the_legacy_session_before_sending_otp() {
    let sandbox = Sandbox::new();
    let mut original = stored("interactive", "old-fixture-token");
    original["supabase.auth.token"] = json!(
        json!({"access_token": "old-fixture-token", "refresh_token": "old-refresh"}).to_string()
    );
    write_config(&sandbox, &original);
    let path = sandbox.config_path();
    let server = Server::start(move |request, _| match request.path.as_str() {
        "/auth/v1/user" => Response::json(json!({"email": EMAIL})),
        "/auth/v1/logout" => {
            assert_eq!(request.method, "POST");
            assert!(request.headers.contains("Bearer old-fixture-token"));
            let saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            assert!(saved.get("supabase.auth.token").is_none());
            Response {
                status: 401,
                ..Response::json(json!({}))
            }
        }
        "/auth/v1/otp" => Response {
            status: 500,
            ..Response::json(json!({}))
        },
        _ => Response::missing(),
    });
    let mut terminal = Terminal::start(command(&sandbox, &server, &["login", "--force"]));
    terminal.wait_for("Choose login method");
    terminal.send("\x1b[B\r");
    terminal.wait_for("Email address");
    terminal.send(&format!("{EMAIL}\r"));
    let (status, output) = terminal.finish();
    assert!(!status.success(), "{output}");
    let paths: Vec<_> = server.requests().into_iter().map(|request| request.path).collect();
    assert_eq!(paths, ["/auth/v1/user", "/auth/v1/logout", "/auth/v1/otp"]);
    let saved: Value = serde_json::from_slice(&fs::read(sandbox.config_path()).unwrap()).unwrap();
    assert_eq!(saved["hcli.credentials"], original["hcli.credentials"]);
    assert!(saved.get("supabase.auth.token").is_none());
}

#[test]
fn named_and_all_logout_require_confirmation_and_clear_associated_session_data() {
    for all in [false, true] {
        let sandbox = Sandbox::new();
        let mut config = stored("interactive", "fixture-token");
        config["supabase.auth.token"] = json!("legacy session fixture");
        config["unrelated"] = json!(42);
        write_config(&sandbox, &config);
        let original = fs::read(sandbox.config_path()).unwrap();
        let args = if all {
            vec!["logout", "--all"]
        } else {
            vec!["logout", "--name", "account"]
        };
        let prompt = if all {
            "Remove all 1 credentials"
        } else {
            "Remove credentials 'account'"
        };
        for confirm in [false, true] {
            let mut terminal = Terminal::start(sandbox.command(&args));
            terminal.wait_for(prompt);
            terminal.send(if confirm {
                "y\r"
            } else {
                "n\r"
            });
            let (status, output) = terminal.finish();
            assert!(status.success(), "{output}");
            if confirm {
                let saved: Value =
                    serde_json::from_slice(&fs::read(sandbox.config_path()).unwrap()).unwrap();
                assert!(saved["hcli.credentials"]["credentials"].as_object().unwrap().is_empty());
                assert!(saved.get("supabase.auth.token").is_none());
                assert_eq!(saved["unrelated"], 42);
            } else {
                assert_eq!(fs::read(sandbox.config_path()).unwrap(), original);
            }
        }
    }
}

#[test]
fn logout_persistence_failure_preserves_credentials_and_legacy_session_together() {
    let sandbox = Sandbox::new();
    let mut config = stored("interactive", "fixture-token");
    config["supabase.auth.token"] = json!("legacy session fixture");
    write_config(&sandbox, &config);
    let path = sandbox.config_path();
    let original = fs::read(&path).unwrap();
    let mut terminal = Terminal::start(sandbox.command(&["logout", "--all"]));
    terminal.wait_for("Remove all 1 credentials");
    let preserved = path.with_extension("saved");
    fs::rename(&path, &preserved).unwrap();
    fs::create_dir(&path).unwrap();
    terminal.send("y\r");
    let (status, output) = terminal.finish();
    assert!(!status.success(), "{output}");
    assert!(!output.contains("Removed 1 credentials"));
    assert_eq!(fs::read(preserved).unwrap(), original);
}

#[test]
fn otp_credentials_are_saved_only_after_remote_user_validation() {
    for valid in [false, true] {
        let sandbox = Sandbox::new();
        let server = Server::start(move |request, _| match request.path.as_str() {
            "/auth/v1/otp" => {
                assert_eq!(
                    serde_json::from_slice::<Value>(&request.body).unwrap(),
                    json!({"email": EMAIL})
                );
                Response::json(json!({}))
            }
            "/auth/v1/verify" => {
                assert_eq!(
                    serde_json::from_slice::<Value>(&request.body).unwrap(),
                    json!({"email": EMAIL, "token": "123456", "type": "email"})
                );
                Response::json(
                    json!({"access_token": "otp-fixture-token", "refresh_token": "fixture-refresh"}),
                )
            }
            "/auth/v1/user" => {
                assert!(request.headers.contains("Bearer otp-fixture-token"));
                Response::json(if valid {
                    json!({"email": EMAIL})
                } else {
                    json!({})
                })
            }
            _ => Response::missing(),
        });
        let mut terminal =
            Terminal::start(command(&sandbox, &server, &["login", "--name", "otp-account"]));
        terminal.wait_for("Choose login method");
        terminal.send("\x1b[B\r");
        terminal.wait_for("Email address");
        terminal.send(&format!("{EMAIL}\r"));
        terminal.wait_for("Enter the code received by email");
        terminal.send("123456\r");
        let (status, output) = terminal.finish();
        assert_eq!(status.success(), valid, "{output}");
        assert_eq!(server.requests().len(), 3);
        if valid {
            let saved: Value =
                serde_json::from_slice(&fs::read(sandbox.config_path()).unwrap()).unwrap();
            assert_eq!(saved["hcli.credentials"]["credentials"]["otp-account"]["email"], EMAIL);
            assert_eq!(
                saved["hcli.credentials"]["credentials"]["otp-account"]["token"],
                "otp-fixture-token"
            );
            assert_eq!(saved["hcli.login.email"], EMAIL);
        } else {
            assert!(!sandbox.config_path().exists());
        }
    }
}
