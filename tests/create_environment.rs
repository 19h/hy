//! Environment creation uses isolated shell fixtures; no installed Python or IDA is changed.
#![cfg(unix)]

#[path = "create_environment/fixture.rs"]
mod fixture;
#[path = "create_environment/reference.rs"]
mod reference;
mod support;

use fixture::Rig;
use serde_json::{Value, json};
use std::fs;
use support::*;

fn report(output: std::process::Output) -> Value {
    assert_success(&output);
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn missing_and_empty_targets_are_created_and_healthy_targets_are_reused() {
    for empty in [false, true] {
        let rig = Rig::new(true);
        if empty {
            fs::create_dir(&rig.target).unwrap();
        }
        let first =
            report(rig.command(true, false).arg("--no-reinstall-plugins").output().unwrap());
        assert_eq!(first["created"], true);
        assert_eq!(first["tool"], "uv");
        assert_eq!(first["plugins_skipped"], false);
        let second = report(rig.command(true, false).output().unwrap());
        assert_eq!(second["created"], false);
        assert_eq!(second["tool"], Value::Null);
        assert_eq!(rig.calls().matches("uv|").count(), 1);
        assert!(!rig.calls().contains("ensurepip"));
    }
}

#[test]
fn unusable_existing_targets_are_preserved_without_creation() {
    for kind in
        ["file", "nonempty", "missing-interpreter", "wrong-version", "no-pip", "broken-interpreter"]
    {
        let rig = Rig::new(true);
        let mut command = rig.command(true, false);
        match kind {
            "file" => fs::write(&rig.target, b"retain").unwrap(),
            "nonempty" => {
                fs::create_dir(&rig.target).unwrap();
                fs::write(rig.target.join("retain"), b"retain").unwrap();
            }
            "missing-interpreter" => {
                fs::create_dir(&rig.target).unwrap();
                fs::write(rig.target.join("pyvenv.cfg"), b"retain").unwrap();
            }
            _ => {
                let executable = rig.existing("python");
                match kind {
                    "wrong-version" => {
                        command.env("HY_TEST_PYTHON_VERSION", "3.12");
                    }
                    "no-pip" => {
                        command.env("HY_TEST_NO_PIP", "1");
                    }
                    _ => fixture::script(&executable, "#!/bin/sh\nexit 1\n"),
                }
            }
        }
        let preserved = if rig.target.is_file() {
            vec![rig.target.clone()]
        } else {
            [
                rig.target.join("retain"),
                rig.target.join("pyvenv.cfg"),
                rig.target.join("bin/python"),
            ]
            .into_iter()
            .filter(|path| path.is_file())
            .collect()
        }
        .into_iter()
        .map(|path| {
            let bytes = fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect::<Vec<_>>();
        let output = command.output().unwrap();
        assert!(!output.status.success(), "{kind}");
        assert!(rig.target.exists());
        assert!(!rig.calls().contains("uv|"));
        for (path, bytes) in preserved {
            assert_eq!(fs::read(path).unwrap(), bytes);
        }
    }
}

#[test]
fn healthy_python3_environment_skips_migration_and_reconfiguration() {
    let rig = Rig::new(true);
    let executable = rig.existing("python3");
    rig.plugin("example", json!(["faildep"]));
    let result = report(
        rig.command(true, true)
            .arg("--no-reinstall-plugins")
            .env("IDAPYTHON_VENV_EXECUTABLE", &executable)
            .output()
            .unwrap(),
    );
    assert_eq!(result["python_exe"], executable.to_str().unwrap());
    assert_eq!(result["plugin_migrations"], json!([]));
    assert_eq!(result["plugins_skipped"], false);
    assert_eq!(result["configured"], false);
    assert!(!rig.calls().contains("-m pip"));
    assert!(!rig.sandbox.path().join(".zprofile").exists());
}

#[test]
fn ida_probe_wins_and_uv_uses_the_registered_interpreter() {
    let rig = Rig::new(true);
    let registered = rig.sandbox.path().join("registered base/bin/python3.12");
    let install = rig.idat(&registered);
    let probe = json!({"executable":registered, "prefix":registered.parent(), "base_prefix":registered.parent(),
        "version_major":3, "version_minor":12, "externally_managed":false, "virtual_env":null});
    let result = report(
        rig.command(true, false)
            .env("HCLI_CURRENT_IDA_INSTALL_DIR", install)
            .env("HY_TEST_IDA_PROBE", probe.to_string())
            .env("HY_TEST_PYTHON_VERSION", "3.12")
            .output()
            .unwrap(),
    );
    assert_eq!(result["python_version"], "3.12");
    assert_eq!(result["python_version_source"], "idat probe");
    assert!(rig.calls().contains(&format!(
        "uv|venv --seed --python {} {}",
        registered.display(),
        rig.target.display()
    )));
}

#[test]
fn path_search_continues_after_wrong_version_and_stdlib_creation_runs_ensurepip() {
    let rig = Rig::new(false);
    fixture::script(&rig.tools.join("python3.13"), "#!/bin/sh\nprintf '3.12\\n'\n");
    fixture::script(&rig.tools.join("python3"), "#!/bin/sh\nexit 1\n");
    rig.python(&rig.tools.join("python"));
    let result = report(rig.command(true, false).output().unwrap());
    assert_eq!(result["tool"], "venv");
    let calls = rig.calls();
    assert!(calls.contains(&format!("/python|-m venv {}", rig.target.display())));
    assert!(calls.contains("/bin/python|-m ensurepip --upgrade"));
}

#[test]
fn ensurepip_failure_prevents_migration_and_configuration() {
    let rig = Rig::new(false);
    rig.python(&rig.tools.join("python3.13"));
    rig.plugin("example", json!(["dependency"]));
    let output = rig.command(true, true).env("HY_TEST_ENSUREPIP_STATUS", "7").output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("exit code 7"));
    assert!(!rig.calls().contains("-m pip"));
    assert!(!rig.sandbox.path().join(".zprofile").exists());
}

#[test]
fn failed_migrations_are_reported_without_failing_environment_creation() {
    let rig = Rig::new(true);
    rig.plugin("first", json!(["faildep"]));
    rig.plugin("second", json!(["dependency"]));
    let result = report(rig.command(true, false).output().unwrap());
    assert_eq!(result["plugin_migrations"][0]["success"], false);
    assert_eq!(result["plugin_migrations"][0]["error"], "fixture dependency failure");
    assert_eq!(result["plugin_migrations"][1]["success"], true);
    assert_eq!(result["plugins_skipped"], false);
    assert!(rig.calls().contains("|-m pip install dependency\n"));
    assert!(!rig.calls().contains("--dry-run"));
    assert_eq!(rig.calls().matches("|-m pip install ").count(), 2);
}

#[test]
fn migration_aborts_when_the_interpreter_disappears_between_plugins() {
    let rig = Rig::new(true);
    rig.plugin("first", json!(["firstdep"]));
    rig.plugin("second", json!(["seconddep"]));
    let template = rig.sandbox.path().join("disappearing-python");
    fixture::script(&template, include_str!("create_environment/disappearing-python.sh"));
    let output = rig.command(true, false).env("HY_TEST_TEMPLATE", &template).output().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("I/O error:"));
    assert!(!rig.target.join("bin/python").exists());
    assert!(rig.target.join("pyvenv.cfg").is_file());
}

#[test]
fn disabling_migration_marks_skipped_only_when_dependencies_exist() {
    let rig = Rig::new(true);
    rig.plugin("example", json!(["dependency"]));
    let result = report(rig.command(true, false).arg("--no-reinstall-plugins").output().unwrap());
    assert_eq!(result["plugins_skipped"], true);
    assert_eq!(result["plugin_migrations"], json!([]));
    assert!(!rig.calls().contains("-m pip"));
}

#[test]
fn interactive_migration_and_configuration_can_be_declined_after_creation() {
    let rig = Rig::new(true);
    rig.plugin("example", json!(["dependency"]));
    let mut terminal = support::terminal::Terminal::start(rig.command(false, true));
    terminal.wait_for("Install these dependencies?");
    terminal.send("n\n");
    terminal.wait_for("Apply these changes?");
    terminal.send("n\n");
    let (status, output) = terminal.finish();
    assert!(status.success(), "{output}");
    assert!(rig.target.join("pyvenv.cfg").is_file());
    assert!(!rig.calls().contains("-m pip"));
    assert!(!rig.sandbox.path().join(".zprofile").exists());
}

#[test]
fn interactive_default_accepts_migration_before_configuration_is_offered() {
    let rig = Rig::new(true);
    rig.plugin("example", json!(["dependency"]));
    let mut terminal = support::terminal::Terminal::start(rig.command(false, true));
    terminal.wait_for("Install these dependencies?");
    terminal.send("yes\n");
    terminal.wait_for("Please enter Y or N");
    terminal.send("\n");
    terminal.wait_for("Apply these changes?");
    assert!(rig.calls().contains("|-m pip install dependency\n"));
    terminal.send("n\n");
    let (status, output) = terminal.finish();
    assert!(status.success(), "{output}");
    assert!(!rig.sandbox.path().join(".zprofile").exists());
}

#[test]
fn creation_and_migration_reports_match_the_upstream_command_body() {
    let mut cases = Vec::new();
    for existing in [false, true] {
        for dependency in [None, Some("dependency"), Some("faildep")] {
            for skip in [false, true] {
                let rig = Rig::new(true);
                if existing {
                    rig.existing("python");
                }
                if let Some(dependency) = dependency {
                    rig.plugin("example", json!([dependency]));
                }
                let mut command = rig.command(true, false);
                if skip {
                    command.arg("--no-reinstall-plugins");
                }
                let mut result = report(command.output().unwrap());
                for migration in result["plugin_migrations"].as_array_mut().unwrap() {
                    migration["error"] = json!(!migration["error"].is_null());
                }
                assert_eq!(result["created"], !existing);
                assert_eq!(result["plugins_skipped"], !existing && dependency.is_some() && skip);
                cases.push(json!({"existing":existing, "dependency":dependency, "skip":skip,
                    "target":rig.target, "expected":result}));
            }
        }
    }
    reference::compare(&cases);
}

#[test]
fn unreadable_dependency_metadata_is_skipped_without_aborting_other_migrations() {
    let rig = Rig::new(true);
    rig.plugin("broken", json!("inline"));
    fs::write(
        rig.sandbox.path().join("idausr/plugins/broken/plugin.py"),
        b"# /// script\n# dependencies = [\n# ///\n",
    )
    .unwrap();
    rig.plugin("working", json!(["dependency"]));
    let result = report(rig.command(true, false).output().unwrap());
    assert_eq!(result["plugin_migrations"].as_array().unwrap().len(), 1);
    assert_eq!(result["plugin_migrations"][0]["name"], "working");
    assert_eq!(result["plugin_migrations"][0]["success"], true);
}
