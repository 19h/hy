use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;

use serde_json::{Value, json};

use super::*;

struct Fixture {
    directory: tempfile::TempDir,
    agent: Agent,
    replies: BTreeMap<String, (String, i32)>,
    checked_status: i32,
}

impl Fixture {
    fn new(kind: Kind, scope: Scope, state: &str, checked_status: i32) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let agent = Agent {
            kind,
            executable: directory.path().join(kind.command()),
        };
        let listing = if state == "installed" || state == "query-failed" {
            match kind {
                Kind::Claude => json!([{"id": PLUGIN_ID, "scope": scope.cli()}]).to_string(),
                Kind::Codex => json!({"plugins": [{"pluginId": PLUGIN_ID}]}).to_string(),
                Kind::Copilot => " • ida-mcp enabled\r\n".into(),
                Kind::Pi => format!(
                    "{} packages:\r\n  git:github.com/HexRaysSA/ida-mcp@latest\r\n",
                    if matches!(scope, Scope::Local) {
                        "Project"
                    } else {
                        "User"
                    }
                ),
                Kind::Omp => json!({"items": [{"name": "ida-mcp"}]}).to_string(),
            }
        } else if state == "malformed" {
            "{invalid JSON".into()
        } else {
            "[]".into()
        };
        let marketplace = if state == "marketplace" {
            if matches!(kind, Kind::Copilot) {
                "◆ HexRaysSA enabled".into()
            } else {
                json!({"marketplaces": [{"name": "HexRaysSA"}]}).to_string()
            }
        } else {
            "[]".into()
        };
        let query = match kind {
            Kind::Claude | Kind::Codex => "plugin list --json".into(),
            Kind::Copilot => "plugin list".into(),
            Kind::Pi => "list --no-approve".into(),
            Kind::Omp => format!("plugin list --json --scope {}", scope.cli()),
        };
        let mut replies = BTreeMap::from([(
            query,
            (
                listing,
                if state == "query-failed" {
                    7
                } else {
                    0
                },
            ),
        )]);
        replies.insert("plugin marketplace list --json".into(), (marketplace.clone(), 0));
        replies.insert("plugin marketplace list".into(), (marketplace, 0));
        let mut script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> {}\ncase \"$*\" in\n",
            quote(directory.path().join("calls").to_str().unwrap())
        );
        for (arguments, (reply, status)) in &replies {
            script.push_str(&format!(
                "{}) printf '%s' {}; exit {status};;\n",
                quote(arguments),
                quote(reply)
            ));
        }
        script.push_str(&format!("*) exit {checked_status};;\nesac\n"));
        fs::write(&agent.executable, script).unwrap();
        fs::set_permissions(&agent.executable, fs::Permissions::from_mode(0o755)).unwrap();
        Self {
            directory,
            agent,
            replies,
            checked_status,
        }
    }

    fn calls(&self) -> Vec<String> {
        fs::read_to_string(self.directory.path().join("calls"))
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn compare_upstream(&self, scope: Scope, success: bool) {
        let Some(python) = std::env::var_os("HY_TEST_MCP_ORACLE_PYTHON") else {
            return;
        };
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut child = Command::new(python)
            .args([
                "-I",
                "-B",
                "-c",
                r#"
import importlib, json, subprocess, sys
from unittest.mock import patch
upstream = importlib.import_module('hcli.commands.mcp.install')
data = json.load(sys.stdin)
calls = []
def run(arguments, **kwargs):
    arguments = arguments[1:]
    calls.append(' '.join(arguments))
    text, status = data['replies'].get(calls[-1], ['', data['checked_status']])
    return subprocess.CompletedProcess(arguments, status, stdout=text, stderr='')
agent = upstream.Agent(data['command'], 'Fixture', '/unused/agent', True)
with patch('subprocess.run', run), patch.object(upstream.console, 'print'):
    try:
        upstream._install_agent(agent, data['scope'])
        success = True
    except upstream.click.ClickException:
        success = False
print(json.dumps({'calls': calls, 'success': success}))
"#,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let data = json!({
            "command": self.agent.kind.command(),
            "scope": if matches!(scope, Scope::Local) { "local" } else { "global" },
            "replies": self.replies,
            "checked_status": self.checked_status,
        });
        child.stdin.take().unwrap().write_all(&serde_json::to_vec(&data).unwrap()).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stdout).unwrap(),
            json!({"calls": self.calls(), "success": success})
        );
    }
}

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[tokio::test]
async fn all_agents_preserve_command_order_scope_and_query_fallbacks() {
    for kind in [Kind::Claude, Kind::Codex, Kind::Copilot, Kind::Pi, Kind::Omp] {
        for scope in [Scope::Global, Scope::Local] {
            if matches!(scope, Scope::Local) && !kind.supports_local() {
                continue;
            }
            for state in ["installed", "marketplace", "absent", "query-failed", "malformed"] {
                let fixture = Fixture::new(kind, scope, state, 0);
                run(&fixture.agent, scope).await.unwrap();
                let calls = fixture.calls();
                let expected_count = match kind {
                    Kind::Claude | Kind::Codex => {
                        if state == "installed" {
                            2
                        } else if state == "marketplace" && matches!(kind, Kind::Claude) {
                            3
                        } else {
                            4
                        }
                    }
                    Kind::Copilot => {
                        if state == "installed" {
                            3
                        } else {
                            4
                        }
                    }
                    Kind::Pi | Kind::Omp => 2,
                };
                assert_eq!(calls.len(), expected_count, "{kind:?} {scope:?} {state}: {calls:?}");
                assert!(!calls.last().unwrap().contains("list"));
                fixture.compare_upstream(scope, true);
            }
        }
    }
}

#[tokio::test]
async fn checked_failures_stop_followup_commands_and_report_agent_status() {
    for kind in [Kind::Claude, Kind::Codex, Kind::Copilot, Kind::Pi, Kind::Omp] {
        let fixture = Fixture::new(kind, Scope::Global, "absent", 23);
        let error = run(&fixture.agent, Scope::Global).await.unwrap_err();
        assert!(matches!(&error, crate::error::Error::Other(_)));
        assert_eq!(error.to_string(), format!("{} exited with status 23", kind.command()));
        fixture.compare_upstream(Scope::Global, false);
    }
}

#[tokio::test]
async fn query_start_errors_abort_instead_of_attempting_installation() {
    let directory = tempfile::tempdir().unwrap();
    let agent = Agent {
        kind: Kind::Codex,
        executable: directory.path().join("missing"),
    };
    let error = run(&agent, Scope::Global).await.unwrap_err();
    assert!(error.to_string().contains("could not start codex"));
}

#[tokio::test]
async fn checked_signal_exit_uses_the_upstream_negative_status() {
    let fixture = Fixture::new(Kind::Codex, Scope::Global, "absent", 0);
    fs::write(&fixture.agent.executable, "#!/bin/sh\nkill -TERM $$\n").unwrap();
    let error = fixture.agent.checked(&["plugin", "add", PLUGIN_ID]).await.unwrap_err();
    assert_eq!(error.to_string(), "codex exited with status -15");
}
