//! MCP orchestration through local release responses and isolated agent shims.
#![cfg(unix)]

mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use serde_json::{Value, json};
use support::{
    http::{Response, Server},
    terminal::Terminal,
    *,
};

struct Fixture {
    sandbox: Sandbox,
    server: Server,
    bin: PathBuf,
    calls: PathBuf,
}

impl Fixture {
    fn new(agent: Option<&str>, fail: bool) -> Self {
        let sandbox = Sandbox::new();
        let bin = sandbox.path().join("bin");
        fs::create_dir(&bin).unwrap();
        let calls = sandbox.path().join("agent-calls");
        if let Some(agent) = agent {
            let executable = bin.join(agent);
            fs::write(
                &executable,
                format!(
                    r#"#!/bin/sh
printf '%s\n' "$*" >> "$MCP_CALLS"
case "$*" in
  *list*) printf '[]\n'; exit 0;;
  *) exit {};;
esac
"#,
                    if fail {
                        23
                    } else {
                        0
                    }
                ),
            )
            .unwrap();
            fs::set_permissions(executable, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut descriptor = identity_manifest("1.0.0", "https://github.com/HexRaysSA/ida-mcp");
        descriptor["plugin"]["name"] = json!("ida-mcp");
        let old = sandbox.path().join("old.zip");
        archive_manifest(&old, &descriptor, &[]);
        assert_success(&sandbox.run(&["plugin", "install", old.to_str().unwrap()]));
        descriptor["plugin"]["version"] = json!("2.0.0");
        let new = sandbox.path().join("new.zip");
        archive_manifest(&new, &descriptor, &[]);
        let bytes = fs::read(new).unwrap();
        let server = Server::start(move |request, base| match request.path.as_str() {
            "/repos/HexRaysSA/ida-mcp/releases/latest" => Response::json(json!({
                "assets": [{"name": "ida-mcp.zip", "browser_download_url": format!("{base}/plugin.zip"), "size": bytes.len()}],
            })),
            "/plugin.zip" => Response::zip(bytes.clone()),
            _ => Response::missing(),
        });
        Self {
            sandbox,
            server,
            bin,
            calls,
        }
    }

    fn command(&self) -> std::process::Command {
        let mut command = self.sandbox.command(&["mcp", "install"]);
        command
            .env("PATH", &self.bin)
            .env("MCP_CALLS", &self.calls)
            .env("GITHUB_API_URL", &self.server.url);
        command
    }

    fn assert_upgraded(&self) {
        let metadata: Value = serde_json::from_slice(
            &fs::read(self.sandbox.path().join("idausr/plugins/ida-mcp/ida-plugin.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(metadata["plugin"]["version"], "2.0.0");
        assert_eq!(self.server.requests().len(), 2);
    }
}

#[test]
fn every_agent_and_scope_is_available_after_upgrading_the_ida_plugin() {
    for (agent, display, scoped, local, expected) in [
        (
            "claude",
            "Claude Code",
            true,
            false,
            "plugin install ida-mcp@HexRaysSA --scope user --yes",
        ),
        (
            "claude",
            "Claude Code",
            true,
            true,
            "plugin install ida-mcp@HexRaysSA --scope project --yes",
        ),
        ("codex.cmd", "Codex CLI", false, false, "plugin add ida-mcp@HexRaysSA"),
        ("copilot", "GitHub Copilot CLI", false, false, "plugin install ida-mcp@HexRaysSA"),
        ("pi", "Pi", true, false, "install git:github.com/HexRaysSA/ida-mcp@latest"),
        ("pi", "Pi", true, true, "install git:github.com/HexRaysSA/ida-mcp@latest --local"),
        (
            "omp",
            "Oh My Pi",
            true,
            false,
            "plugin install github:HexRaysSA/ida-mcp#latest --scope user",
        ),
        (
            "omp",
            "Oh My Pi",
            true,
            true,
            "plugin install github:HexRaysSA/ida-mcp#latest --scope project",
        ),
    ] {
        let fixture = Fixture::new(Some(agent), false);
        let mut terminal = Terminal::start(fixture.command());
        terminal.wait_for("Select an agent:");
        terminal.wait_for(&format!("{display} ({agent})"));
        terminal.send("\n");
        if scoped {
            terminal.wait_for("Select installation scope:");
            terminal.send(if local {
                "\x1b[A\n"
            } else {
                "\n"
            });
        }
        let (status, output) = terminal.finish();
        assert!(status.success(), "{agent}: {output}");
        let location = if local {
            "in this repository"
        } else {
            "for the current user"
        };
        assert!(
            output.contains(&format!("Installed IDA MCP for {display} {location}.")),
            "{output}"
        );
        let calls = fs::read_to_string(&fixture.calls).unwrap();
        assert_eq!(calls.lines().last(), Some(expected), "{agent}: {calls}");
        fixture.assert_upgraded();
    }
}

#[test]
fn selection_cancellation_and_missing_agents_preserve_the_completed_plugin_upgrade() {
    for stage in ["agent", "scope", "missing"] {
        let fixture = Fixture::new((stage != "missing").then_some("claude"), false);
        if stage == "missing" {
            let output = fixture.command().output().unwrap();
            assert_eq!(output.status.code(), Some(1));
            assert!(
                String::from_utf8_lossy(&output.stderr)
                    .contains("no supported agent command found")
            );
        } else {
            let mut terminal = Terminal::start(fixture.command());
            terminal.wait_for("Select an agent:");
            if stage == "scope" {
                terminal.send("\n");
                terminal.wait_for("Select installation scope:");
            }
            terminal.send("\x1b");
            let (status, _) = terminal.finish();
            assert_eq!(status.code(), Some(1));
        }
        assert!(!fixture.calls.exists());
        fixture.assert_upgraded();
    }
}

#[test]
fn agent_failure_returns_command_error_without_undoing_the_plugin_upgrade() {
    let fixture = Fixture::new(Some("codex"), true);
    let mut terminal = Terminal::start(fixture.command());
    terminal.wait_for("Select an agent:");
    terminal.send("\n");
    let (status, output) = terminal.finish();
    assert_eq!(status.code(), Some(1));
    assert!(output.contains("codex exited with status 23"), "{output}");
    let calls = fs::read_to_string(&fixture.calls).unwrap();
    assert_eq!(calls.lines().count(), 3);
    assert!(!calls.contains("plugin add"));
    fixture.assert_upgraded();
}

#[test]
fn plugin_download_failure_prevents_agent_discovery_and_execution() {
    let fixture = Fixture::new(Some("codex"), false);
    let unavailable = Server::start(|_, _| Response::missing());
    let output = fixture.command().env("GITHUB_API_URL", &unavailable.url).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(!fixture.calls.exists());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("Select an agent:"));
    let metadata: Value = serde_json::from_slice(
        &fs::read(fixture.sandbox.path().join("idausr/plugins/ida-mcp/ida-plugin.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(metadata["plugin"]["version"], "1.0.0");
    assert_eq!(unavailable.requests().len(), 1);
}
