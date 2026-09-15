//! CLI selection precedence and successful-result-only IDA probe caching.
#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

#[allow(dead_code)]
#[path = "python_environment_guard/fixture.rs"]
mod fixture;
mod support;

use fixture::{Fixture, executable};
use support::assert_success;

struct Rig {
    fixture: Fixture,
    base: PathBuf,
}

impl Rig {
    fn new() -> Self {
        let fixture = Fixture::new(true);
        let base = fixture.sandbox.path().join("base");
        for path in [
            fixture.python.clone(),
            base.join("bin/python3.13"),
            base.join("bin/python3"),
            base.join("bin/python"),
        ] {
            executable(&path, "#!/bin/sh\nprintf 'selected:%s\n' \"$0\"\nprintf '<%s>\n' \"$@\"\n");
        }
        Self {
            fixture,
            base,
        }
    }

    fn probe(&self) -> Value {
        json!({
            "frozen": false, "executable": self.fixture.installation.join("idat"),
            "prefix": self.base, "base_prefix": self.base, "version_major": 3, "version_minor": 13,
            "pip_available": true, "externally_managed": false, "scripts": self.base.join("bin"),
        })
    }

    fn command(&self, probe: &Value) -> Command {
        let mut command = self.fixture.command(&[
            "ida",
            "python",
            "--no-python-environment-check",
            "exec",
            "--child-argument",
        ]);
        command
            .env_remove("HCLI_CURRENT_IDA_PYTHON_EXE")
            .env_remove("IDAPYTHON_VENV_EXECUTABLE")
            .env("HY_TEST_IDA_PROBE", probe.to_string());
        command
    }
}

fn assert_selected(output: &std::process::Output, expected: &Path) {
    assert_success(output);
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .starts_with(&format!("selected:{}\n", expected.display())),
        "{output:?}"
    );
}

#[test]
fn derivation_selects_versioned_candidates_and_requested_venvs_before_base_fallback() {
    for scenario in [
        "base",
        "no-versioned",
        "requested",
        "active-venv",
        "different-prefix",
        "unvalidated-executable",
        "null-executable",
    ] {
        let rig = Rig::new();
        let mut probe = rig.probe();
        let mut expected = rig.base.join("bin/python3.13");
        match scenario {
            "no-versioned" => {
                fs::remove_file(&expected).unwrap();
                expected = rig.base.join("bin/python3");
            }
            "requested" => {
                probe["idapython_venv_executable"] = json!(rig.fixture.python);
                expected = rig.fixture.python.clone();
            }
            "active-venv" => {
                probe["executable"] = json!(rig.fixture.python);
                probe["virtual_env"] = json!(rig.fixture.python.parent().unwrap().parent());
                expected = rig.fixture.python.clone();
            }
            "different-prefix" => {
                probe["prefix"] = json!(rig.fixture.python.parent().unwrap().parent());
                expected = rig.fixture.python.clone();
            }
            "unvalidated-executable" => {
                probe["executable"] = json!(rig.fixture.python);
            }
            "null-executable" => {
                probe["executable"] = Value::Null;
            }
            _ => (),
        }
        assert_selected(&rig.command(&probe).output().unwrap(), &expected);
        assert_eq!(rig.fixture.calls(), "idat\n", "{scenario}");
    }
}

#[test]
fn overrides_preserve_unchecked_hcli_selection_and_validate_the_idapython_variable() {
    for override_mode in ["absent", "empty", "existing", "missing"] {
        for variable_mode in ["absent", "existing", "directory"] {
            let rig = Rig::new();
            let mut command = rig.command(&rig.probe());
            let missing = rig.fixture.sandbox.path().join("missing-python");
            match override_mode {
                "empty" => {
                    command.env("HCLI_CURRENT_IDA_PYTHON_EXE", "");
                }
                "existing" => {
                    command.env("HCLI_CURRENT_IDA_PYTHON_EXE", &rig.fixture.python);
                }
                "missing" => {
                    command.env("HCLI_CURRENT_IDA_PYTHON_EXE", missing);
                }
                _ => (),
            }
            match variable_mode {
                "existing" => {
                    command.env("IDAPYTHON_VENV_EXECUTABLE", &rig.fixture.python);
                }
                "directory" => {
                    command.env("IDAPYTHON_VENV_EXECUTABLE", &rig.base);
                }
                _ => (),
            }
            let output = command.output().unwrap();
            let should_probe =
                matches!(override_mode, "absent" | "empty") && variable_mode != "existing";
            assert_eq!(rig.fixture.calls().contains("idat"), should_probe);
            if override_mode == "missing" {
                assert_eq!(output.status.code(), Some(1));
                assert!(String::from_utf8_lossy(&output.stderr).contains("I/O error"));
            } else {
                let expected = if should_probe {
                    rig.base.join("bin/python3.13")
                } else {
                    rig.fixture.python.clone()
                };
                assert_selected(&output, &expected);
            }
        }
    }
}

#[test]
fn frozen_and_unresolvable_probes_report_source_diagnostics_before_execution() {
    for frozen in [false, true] {
        let rig = Rig::new();
        let probe = json!({
            "frozen": frozen, "executable": null, "prefix": "", "base_prefix": "",
            "version_major": 3, "version_minor": 13, "externally_managed": false,
        });
        let output = rig.command(&probe).env("HCLI_BINARY_NAME", "fixture-hy").output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        if frozen {
            assert!(stderr.contains(
                "IDA is running as a frozen application, cannot detect Python executable"
            ));
        } else {
            assert!(stderr.contains("Run 'fixture-hy ida python create-environment'"), "{stderr}");
            assert!(stderr.contains("sys.executable: None\nVIRTUAL_ENV: None\nIDAPYTHON_VENV_EXECUTABLE: None\nTried: []"), "{stderr}");
        }
        assert_eq!(rig.fixture.calls(), "idat\n");
    }
}

#[test]
fn failed_ida_probes_retry_but_successful_probes_are_cached_within_one_command() {
    for fail_first in [false, true] {
        let rig = Rig::new();
        executable(
            &rig.fixture.installation.join("idat"),
            include_str!("python_resolution/idat.sh"),
        );
        let output = rig
            .fixture
            .command(&["ida", "python", "explain-environment", "--json"])
            .env_remove("HCLI_CURRENT_IDA_PYTHON_EXE")
            .env_remove("IDAPYTHON_VENV_EXECUTABLE")
            .env("HY_TEST_IDA_PROBE", rig.probe().to_string())
            .env(
                "HY_TEST_FAIL_FIRST",
                if fail_first {
                    "1"
                } else {
                    "0"
                },
            )
            .output()
            .unwrap();
        assert_success(&output);
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            rig.fixture.calls().lines().filter(|line| *line == "idat").count(),
            if fail_first {
                2
            } else {
                1
            }
        );
        assert!(report["python_environment"]["idat_probe"].is_object(), "{report}");
        let error = &report["python_environment"]["python_exe_error"];
        if fail_first {
            assert_eq!(
                error,
                "PythonNotFoundError: failed to run idat to detect IDA's Python interpreter. If you know the interpreter path, set HCLI_CURRENT_IDA_PYTHON_EXE=/path/to/python and try again."
            );
        } else {
            assert!(error.is_null(), "{report}");
        }
    }
}
