//! Native doctor fixtures operate only on owned files and shell stand-ins.
#![cfg(unix)]

use std::fs;
use std::process::Command;

use serde_json::{Value, json};

#[allow(dead_code)]
#[path = "create_environment/fixture.rs"]
mod fixture;
#[path = "python_doctor/reference.rs"]
mod reference;
mod support;

use fixture::{Rig, script};

fn command(rig: &Rig, json_output: bool) -> Command {
    let args = if json_output {
        vec!["ida", "python", "doctor", "--json"]
    } else {
        vec!["ida", "python", "doctor"]
    };
    let mut command = rig.sandbox.command(&args);
    command
        .env("PATH", &rig.tools)
        .env("HY_TEST_EVENTS", &rig.events)
        .env_remove("VIRTUAL_ENV")
        .env_remove("UV_CACHE_DIR")
        .env("HCLI_CURRENT_IDA_INSTALL_DIR", rig.sandbox.path().join("missing-ida"));
    command
}

fn report(command: &mut Command) -> Value {
    let output = command.output().unwrap();
    let report: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{error}: {output:?}"));
    assert_eq!(output.status.success(), report["ok"] == true, "{report}");
    report
}

fn ids(report: &Value) -> Vec<&str> {
    report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| finding["id"].as_str().unwrap())
        .collect()
}

#[test]
fn explicit_override_skips_ida_and_keeps_independent_version_and_pip_observations() {
    for missing_pip in [false, true] {
        let rig = Rig::new(false);
        let executable = rig.existing("python");
        let install = rig.sandbox.path().join("ida");
        script(
            &install.join("idat"),
            "#!/bin/sh\necho unexpected-idat >> \"$HY_TEST_EVENTS\"\nexit 1\n",
        );
        let result = report(
            command(&rig, true)
                .env("HCLI_CURRENT_IDA_PYTHON_EXE", &executable)
                .env("HCLI_CURRENT_IDA_INSTALL_DIR", &install)
                .env("HY_TEST_PYTHON_VERSION", "3.9")
                .env(
                    "HY_TEST_NO_PIP",
                    if missing_pip {
                        "1"
                    } else {
                        "0"
                    },
                ),
        );
        assert_eq!(result["python_version"], "3.9");
        assert_eq!(result["ida_python_version"], Value::Null);
        assert_eq!(result["pip_available"], !missing_pip);
        assert_eq!(result["pattern"]["id"], "venv-not-configured");
        assert_eq!(result["ok"], !missing_pip);
        assert_eq!(
            ids(&result),
            if missing_pip {
                vec!["hcli-override-active", "no-pip", "no-venv-exe-var"]
            } else {
                vec!["hcli-override-active", "no-venv-exe-var"]
            }
        );
        assert!(!rig.calls().contains("unexpected-idat"));
        assert!(rig.calls().contains("import pip") && rig.calls().contains("sys.version_info"));
        assert!(!rig.calls().contains("__hy__"));
    }
}

#[test]
fn configured_environment_checks_embedded_version_and_reports_startup_activation() {
    for version in ["3.13", "3.12"] {
        let rig = Rig::new(false);
        let executable = rig.existing("python");
        let install = rig.idat(&rig.sandbox.path().join("registered/bin/python"));
        let rc = rig.sandbox.path().join("idausr/idapythonrc.py");
        fs::create_dir_all(rc.parent().unwrap()).unwrap();
        fs::write(&rc, b"# addsitedir is a textual hint, not executed\n").unwrap();
        let probe = json!({
            "executable": "idat", "prefix": "base", "base_prefix": "base",
            "version_major": 3, "version_minor": version.split('.').nth(1).unwrap().parse::<i64>().unwrap(),
            "pip_available": true, "externally_managed": false, "scripts": "bin",
        });
        let result = report(
            command(&rig, true)
                .env_remove("HCLI_CURRENT_IDA_PYTHON_EXE")
                .env("IDAPYTHON_VENV_EXECUTABLE", &executable)
                .env("HCLI_CURRENT_IDA_INSTALL_DIR", &install)
                .env("HY_TEST_IDA_PROBE", probe.to_string()),
        );
        assert_eq!(result["ida_python_version"], version);
        assert_eq!(result["pattern"]["id"], "idapythonrc-venv");
        assert_eq!(
            ids(&result),
            if version == "3.13" {
                vec!["idapythonrc-venv"]
            } else {
                vec!["version-mismatch", "idapythonrc-venv"]
            }
        );
        assert_eq!(fs::read(&rc).unwrap(), b"# addsitedir is a textual hint, not executed\n");
    }
}

#[test]
fn unusable_interpreters_preserve_missing_versus_unrunnable_classifications() {
    for exists in [false, true] {
        let rig = Rig::new(false);
        let executable = rig.target.join("bin/python");
        if exists {
            script(&executable, "#!/bin/sh\nexit 1\n");
        }
        let result = report(command(&rig, true).env("HCLI_CURRENT_IDA_PYTHON_EXE", &executable));
        assert_eq!(result["python_version"], Value::Null);
        assert_eq!(result["pip_available"], false);
        assert_eq!(
            ids(&result),
            if exists {
                vec!["hcli-override-active", "no-venv", "no-pip"]
            } else {
                vec!["hcli-override-active", "python-exe-not-found"]
            }
        );
        assert_eq!(
            result["pattern"]["id"],
            if exists {
                "default"
            } else {
                "missing-interpreter"
            }
        );
    }
}

#[test]
fn unresolved_python_has_its_own_findings_and_no_setup_or_context_notes() {
    let rig = Rig::new(false);
    let result = report(
        command(&rig, true)
            .env_remove("HCLI_CURRENT_IDA_PYTHON_EXE")
            .env("IDAPYTHON_VENV_EXECUTABLE", rig.target.join("missing")),
    );
    assert_eq!(ids(&result), vec!["venv-exe-not-found", "python-not-found"]);
    assert_eq!(result["pattern"], Value::Null);
    assert_eq!(result["notes"], json!([]));
    assert_eq!(result["python_exe"], Value::Null);
}

#[test]
fn doctor_plain_text_matches_upstream_report_renderer() {
    let mut cases = Vec::new();
    for scenario in 0..8 {
        let rig = Rig::new(scenario == 1);
        let executable = rig.existing("python");
        if scenario == 2 {
            fs::remove_file(rig.target.join("pyvenv.cfg")).unwrap();
        }
        if scenario == 3 {
            fs::remove_file(&executable).unwrap();
        }
        if scenario == 4 {
            fs::create_dir_all(rig.sandbox.path().join("idausr/venv")).unwrap();
            fs::write(
                rig.sandbox.path().join("idausr/venv/pyvenv.cfg"),
                "version_info = 03.013.7\n",
            )
            .unwrap();
        }
        let configure = |command: &mut Command| {
            command.env("HCLI_CURRENT_IDA_PYTHON_EXE", &executable).env(
                "HY_TEST_NO_PIP",
                if scenario == 1 {
                    "1"
                } else {
                    "0"
                },
            );
            if scenario == 5 {
                command
                    .env_remove("HCLI_CURRENT_IDA_PYTHON_EXE")
                    .env("IDAPYTHON_VENV_EXECUTABLE", &executable);
            }
            if scenario == 6 {
                command.env_remove("HCLI_CURRENT_IDA_PYTHON_EXE");
            }
            if scenario == 7 {
                command.env("HCLI_BINARY_NAME", "hy-fixture");
            }
        };
        let mut json_command = command(&rig, true);
        configure(&mut json_command);
        let report = report(&mut json_command);
        let mut text_command = command(&rig, false);
        configure(&mut text_command);
        let output = text_command.output().unwrap();
        assert_eq!(output.status.success(), report["ok"] == true);
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("IDA installation\n"));
        assert_eq!(text.contains("Setup:"), !report["pattern"].is_null());
        assert_eq!(text.contains("Fix:\n"), !report["findings"].as_array().unwrap().is_empty());
        cases.push(json!({
            "report": report, "expected": text,
            "binary": if scenario == 7 { "hy-fixture" } else { "hy" },
        }));
    }
    reference::compare(&cases);
}

#[test]
fn managed_base_interpreters_skip_pip_only_when_the_resolution_probe_supplies_the_marker() {
    for managed in [false, true] {
        let rig = Rig::new(false);
        let registered = rig.sandbox.path().join("registered/bin/python");
        let install = rig.idat(&registered);
        let prefix = registered.parent().unwrap().parent().unwrap();
        let probe = json!({
            "executable": "idat", "prefix": prefix, "base_prefix": prefix, "version_major": 3, "version_minor": 13,
            "pip_available": false, "externally_managed": managed, "scripts": "bin",
        });
        let result = report(
            command(&rig, true)
                .env_remove("HCLI_CURRENT_IDA_PYTHON_EXE")
                .env("HCLI_CURRENT_IDA_INSTALL_DIR", &install)
                .env("HY_TEST_IDA_PROBE", probe.to_string())
                .env("HY_TEST_NO_PIP", "1"),
        );
        assert_eq!(result["externally_managed"], managed);
        assert_eq!(
            result["pip_available"],
            if managed {
                Value::Null
            } else {
                json!(false)
            }
        );
        assert_eq!(
            ids(&result),
            if managed {
                vec!["no-venv", "externally-managed"]
            } else {
                vec!["no-venv", "no-pip"]
            }
        );
        assert_eq!(rig.calls().contains("import pip"), !managed);
    }
}

#[test]
fn filesystem_observations_identify_uv_overlays_shell_activation_and_conda() {
    for scenario in ["overlay", "archive", "cache", "shell", "conda", "ordinary"] {
        let rig = Rig::new(false);
        let root = if scenario == "archive" {
            rig.sandbox.path().join("archive-v0/environment")
        } else {
            rig.target.clone()
        };
        let executable = root.join("bin/python");
        rig.python(&executable);
        if scenario == "conda" {
            fs::create_dir_all(root.join("conda-meta")).unwrap();
        } else {
            fs::write(
                root.join("pyvenv.cfg"),
                if scenario == "overlay" {
                    "EXTENDS-ENVIRONMENT = /fixture/base\n"
                } else {
                    "home = fixture\n"
                },
            )
            .unwrap();
        }
        let mut command = command(&rig, true);
        command.env("HCLI_CURRENT_IDA_PYTHON_EXE", &executable);
        if scenario == "cache" {
            command.env("UV_CACHE_DIR", &root);
        }
        if scenario == "shell" {
            command.env("VIRTUAL_ENV", "/fixture/unrelated-shell");
        }
        let result = report(&mut command);
        let pattern = match scenario {
            "overlay" | "archive" | "cache" => "uv-ephemeral",
            "shell" => "shell-activated-venv",
            "conda" => "conda",
            _ => "venv-not-configured",
        };
        assert_eq!(result["pattern"]["id"], pattern, "{scenario}: {result}");
        if scenario == "conda" {
            assert!(
                result["findings"][1]["detail"]
                    .as_str()
                    .unwrap()
                    .contains("conda environments have no pyvenv.cfg")
            );
        }
    }
}
