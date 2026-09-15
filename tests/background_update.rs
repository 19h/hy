//! Release-profile CLI coverage for the binary-only background update hook.

mod support;

use std::fs;
use std::process::Command;

use serde_json::{Value, json};
use support::{
    http::{Response, Server},
    *,
};

fn command_with_updates(sandbox: &Sandbox) -> Command {
    let isolated = sandbox.command(&["commands"]);
    let mut command = Command::new(isolated.get_program());
    command.args(isolated.get_args().filter(|argument| *argument != "--disable-updates"));
    for (key, value) in isolated.get_envs() {
        if let Some(value) = value {
            command.env(key, value);
        } else {
            command.env_remove(key);
        }
    }
    command.env_remove("CARGO").env_remove("HCLI_DISABLE_UPDATES");
    command
}

fn cache_path(sandbox: &Sandbox) -> std::path::PathBuf {
    let directory = if cfg!(target_os = "macos") {
        sandbox.path().join("Library/Caches/fixture-hcli")
    } else if cfg!(windows) {
        sandbox.path().join("local/hex-rays/fixture-hcli/Cache")
    } else {
        sandbox.path().join("cache/fixture-hcli")
    };
    directory.join("update_check.json")
}

#[test]
#[ignore = "run with cargo test --release --test background_update -- --ignored"]
#[allow(
    clippy::assertions_on_constants,
    reason = "This ignored test must reject a debug-profile invocation."
)]
fn background_cli_discovers_pages_saves_results_and_respects_recent_cache() {
    assert!(!cfg!(debug_assertions), "background checking requires a release-profile binary");
    for scenario in ["stable", "development", "current", "failure"] {
        let sandbox = Sandbox::new();
        let server = Server::start(move |request, _| {
            if scenario == "failure" {
                return Response::missing();
            }
            match request.path.as_str() {
                "/repos/19h/hy/releases?per_page=100&page=1" => {
                    Response::json(json!(vec![json!({"tag_name": "0.1.0"}); 100]))
                }
                "/repos/19h/hy/releases?per_page=100&page=2" => {
                    if scenario == "current" {
                        Response::json(json!([]))
                    } else {
                        Response::json(json!([
                            {"tag_name": "v2.0.0", "draft": true},
                            {"tag_name": "3.0.0-rc.1"},
                        ]))
                    }
                }
                _ => Response::missing(),
            }
        });
        let current = if scenario == "development" {
            "1.0.0-dev.1"
        } else {
            "1.0.0"
        };
        let mut command = command_with_updates(&sandbox);
        command
            .env("HCLI_BINARY_NAME", "fixture-hcli")
            .env("HCLI_VERSION", current)
            .env("GITHUB_API_URL", &server.url);
        let output = command.output().unwrap();
        assert_success(&output);
        let report = String::from_utf8_lossy(&output.stdout);
        let expected = match scenario {
            "development" => Some("3.0.0-rc.1"),
            "stable" => Some("2.0.0"),
            _ => None,
        };
        let path = cache_path(&sandbox);
        if scenario == "failure" {
            assert!(!path.exists());
            assert!(!report.contains("Update available!"));
            assert!(!report.contains("You have the latest version"));
            assert_eq!(server.requests().len(), 1);
            continue;
        }
        if let Some(latest) = expected {
            assert!(report.contains(&format!("{current} -> {latest}")), "{report}");
            assert!(report.contains("Run fixture-hcli update to update"));
        } else {
            assert!(report.contains("You have the latest version 1.0.0!"));
        }
        let bytes = fs::read(&path).unwrap();
        let data: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(data["latest_version"], json!(expected));
        assert_eq!(server.requests().len(), 2);

        let cached = command.output().unwrap();
        assert_success(&cached);
        let report = String::from_utf8_lossy(&cached.stdout);
        assert!(!report.contains("Update available!"));
        assert!(!report.contains("You have the latest version"));
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(server.requests().len(), 2);
    }
}
