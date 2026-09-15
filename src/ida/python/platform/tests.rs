use std::path::PathBuf;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::detection::{Context, Session, System};
use super::plan::{Action, Kind, Plan};

mod reference;

#[test]
fn platform_plans_match_upstream_steps_warnings_and_manual_instructions() {
    let mut cases = Vec::new();
    for system in [System::Windows, System::Macos, System::Linux] {
        for shell in [
            None,
            Some(""),
            Some("/bin/bash"),
            Some("/bin/zsh/"),
            Some("/bin/fish"),
            Some("/bin/sh"),
            Some("/bin/notbash"),
        ] {
            for systemd in [false, true] {
                for session in [Session::Wayland, Session::X11, Session::Tty, Session::Unknown] {
                    for value in [
                        "/venv/bin/python".into(),
                        "/path with 'quotes'/\"{name}&{value}\"/python\n".into(),
                        "x".repeat(2048),
                    ] {
                        let context = Context {
                            system,
                            home: PathBuf::from("/fixture/home"),
                            shell: shell.map(String::from),
                            systemd,
                            session,
                        };
                        let plan = super::build(&context, "IDAPYTHON_VENV_EXECUTABLE", &value);
                        let expected = json!({
                            "plan": plan_snapshot(&plan),
                            "needs_logout": plan.needs_logout(),
                            "shell": context.shell_kind(),
                        });
                        cases.push(json!({
                            "system": format!("{system:?}"),
                            "shell": shell,
                            "systemd": systemd,
                            "session": format!("{session:?}").to_lowercase(),
                            "value": value,
                            "expected": expected,
                        }));
                    }
                }
            }
        }
    }
    assert_eq!(cases.len(), 504);
    verify_digest(&cases, "9b2d7a8899648b748934458b4260276eb73d54488ed8373305f6957ccb0082c6");
    reference::compare("plans", &cases);
}

#[test]
fn session_detection_uses_nonempty_wayland_display_term_precedence() {
    let mut cases = Vec::new();
    for wayland in [None, Some(""), Some("fixture")] {
        for display in [None, Some(""), Some("fixture")] {
            for term in [None, Some(""), Some("fixture")] {
                let expected = format!("{:?}", super::detection::session(wayland, display, term))
                    .to_lowercase();
                cases.push(json!({
                    "wayland": wayland,
                    "display": display,
                    "term": term,
                    "expected": expected,
                }));
            }
        }
    }
    verify_digest(&cases, "bbbd76b43e0a411b55a66997d1e790e21ef5bd65a70e094715027004f5abaaff");
    reference::compare("sessions", &cases);
}

#[test]
fn file_updates_match_source_without_rewriting_skipped_files() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("configuration");
    let mut cases = Vec::new();
    for kind in [Kind::ShellProfile, Kind::LinuxEnvironmentD] {
        for content in
            ["export FOO=\"/new\"", "set -gx FOO \"/new\"", "set -gx FOO \"a=b\"", "FOO=/new\n"]
        {
            let mut inputs = vec![None, Some(Vec::new())];
            inputs.extend(
                [
                    "# retain\n",
                    "unrelated",
                    "export FOO=old\n",
                    "export FOO=old\n# middle\nexport FOO=older\n# end\n",
                    "  export FOO=old\n",
                    "set -gx OTHER old\nset -gx FOO old\n",
                    "export FOO=old\r\n# CRLF\r\n",
                    "# CR\rexport FOO=old\r",
                    "# split\u{85}export FOO=old\u{2028}# end",
                    "\x0bexport FOO=old\x0c",
                    "\x1cexport FOO=old\x1d\x1e",
                ]
                .into_iter()
                .map(|value| Some(value.as_bytes().to_vec())),
            );
            inputs.extend([
                Some(content.as_bytes().to_vec()),
                Some(format!("# before\n{content}\n# after\n").into_bytes()),
                Some(format!("export FOO=stale\n{content}\n").into_bytes()),
                Some(format!("prefix{content}suffix").into_bytes()),
                Some([b"# invalid \xff\xfe\n".as_slice(), content.as_bytes(), b"\n"].concat()),
                Some(b"# invalid \xe2\x82\nexport FOO=old\n".to_vec()),
            ]);
            for initial in inputs {
                let _ = std::fs::remove_file(&path);
                if let Some(bytes) = &initial {
                    std::fs::write(&path, bytes).unwrap();
                }
                let (skipped, message) = super::files::apply(kind, &path, content).unwrap();
                let expected = json!({
                    "skipped": skipped,
                    "data": std::fs::read(&path).unwrap(),
                    "message": message.replace(path.to_str().unwrap(), "<path>"),
                });
                if skipped {
                    assert_eq!(expected["data"], json!(initial.as_ref().unwrap()));
                }
                cases.push(json!({
                    "kind": kind,
                    "initial": initial,
                    "content": content,
                    "expected": expected,
                }));
            }
        }
    }
    verify_digest(&cases, "7b526a38c5037a9b99cb729e1f53a6cb501e2126d2accc131334ee414e6d692c");
    reference::compare("files", &cases);
}

// Adapt the native operation enum to the source dataclass only at the oracle boundary.
fn plan_snapshot(plan: &Plan) -> Value {
    let steps: Vec<_> = plan
        .steps
        .iter()
        .map(|step| {
            let (path, content, command) = match &step.action {
                Action::File {
                    path,
                    content,
                } => (json!(path), json!(content), Value::Null),
                Action::Command {
                    program,
                    args,
                } => (
                    Value::Null,
                    Value::Null,
                    json!(std::iter::once(program).chain(args).collect::<Vec<_>>()),
                ),
            };
            json!({
                "kind": step.kind,
                "description": step.description,
                "file_path": path,
                "file_content": content,
                "command": command,
                "needs_logout": step.needs_logout,
            })
        })
        .collect();
    json!({
        "steps": steps,
        "warnings": plan.warnings,
        "env_var_name": plan.env_var_name,
        "env_var_value": plan.env_var_value,
        "manual_instructions": plan.manual_instructions,
    })
}

fn verify_digest(cases: &[Value], expected_digest: &str) {
    let mut expected = json!(cases.iter().map(|case| &case["expected"]).collect::<Vec<_>>());
    canonicalize(&mut expected);
    let digest = format!("{:x}", Sha256::digest(serde_json::to_vec(&expected).unwrap()));
    assert_eq!(digest, expected_digest);
}

fn canonicalize(value: &mut Value) {
    match value {
        Value::String(text) => *text = text.replace('\\', "/"),
        Value::Array(values) => values.iter_mut().for_each(canonicalize),
        Value::Object(values) => {
            if let Some(Value::Array(data)) = values.get_mut("data") {
                let mut normalized = Vec::new();
                for (index, byte) in data.iter().enumerate() {
                    if byte == 13 && data.get(index + 1).is_some_and(|next| next == 10) {
                        continue;
                    }
                    normalized.push(byte.clone());
                }
                *data = normalized;
            }
            values.values_mut().for_each(canonicalize);
        }
        _ => (),
    }
}
