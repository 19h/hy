//! Version text failures propagate or are caught at each source caller's boundary.
#![cfg(unix)]

use std::fs;
use std::process::Stdio;

use serde_json::Value;

#[path = "python_version/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "python_environment_guard/fixture.rs"]
mod guard_fixture;
mod support;

use fixture::Rig;
use support::assert_success;

const DECODE_ERROR: &str = "'utf-8' codec can't decode byte 0xff in position 0: invalid start byte";

#[test]
fn version_probe_inherits_stdin_without_consuming_the_childs_remaining_input() {
    let rig = Rig::new();
    let input = rig.fixture.sandbox.path().join("stdin");
    fs::write(&input, "probe-line\nchild-line\n").unwrap();
    let output = rig
        .command(&["ida", "python", "exec", "-c", "fixture"])
        .stdin(Stdio::from(fs::File::open(input).unwrap()))
        .env("HY_TEST_READ_INPUT", "1")
        .env("PYTHONHOME", "fixture-home")
        .env("VIRTUAL_ENV", "fixture-venv")
        .env("PYTHONUTF8", "0")
        .output()
        .unwrap();
    assert_success(&output);
    assert!(rig.fixture.calls().contains("version:fixture-home|fixture-venv|0\n"));
    assert!(rig.fixture.calls().contains("version-input:probe-line\n"));
    assert_eq!(output.stdout, b"child-input:child-line\n");
}

#[test]
fn doctor_explain_and_bundle_propagate_decode_errors_before_status_handling() {
    for command in ["doctor", "explain-environment", "bundle"] {
        for status in [0, 7] {
            for bad_stdout in [false, true] {
                let rig = Rig::new();
                if bad_stdout {
                    rig.streams(b"\xff", b"");
                } else {
                    rig.streams(b"3.12\n", b"\xff");
                }
                let mut invocation = if command == "bundle" {
                    rig.bundle_command()
                } else {
                    rig.command(&["ida", "python", command, "--json"])
                };
                let output =
                    invocation.env("HY_TEST_VERSION_STATUS", status.to_string()).output().unwrap();
                assert!(!output.status.success());
                assert!(
                    String::from_utf8_lossy(&output.stderr).contains(DECODE_ERROR),
                    "{output:?}"
                );
                assert!(output.stdout.is_empty());
                assert!(!rig.bundle.exists());
            }
        }
    }
}

#[test]
fn guard_decode_errors_skip_diagnostics_but_keep_the_mandatory_pip_check() {
    for install in [false, true] {
        for pip_status in [0, 1] {
            let rig = Rig::new();
            rig.streams(b"3.12", b"\xff");
            let args = if install {
                vec!["plugin", "install", rig.package.to_str().unwrap()]
            } else {
                vec!["ida", "python", "exec", "-c", "fixture"]
            };
            let output = rig
                .command(&args)
                .env("HY_TEST_PIP_STATUS", pip_status.to_string())
                .output()
                .unwrap();
            assert_eq!(output.status.success(), !install || pip_status == 0, "{output:?}");
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(!stderr.contains(DECODE_ERROR));
            assert!(!stderr.contains("overrides normal Python detection"));
            if install {
                assert_eq!(stderr.contains("pip is not available"), pip_status != 0);
                assert_eq!(rig.fixture.calls().matches("pip-check\n").count(), 2);
                assert_eq!(
                    rig.fixture.sandbox.path().join("idausr/plugins/example").is_dir(),
                    pip_status == 0
                );
            } else {
                assert!(rig.fixture.calls().contains("child-run\n"));
            }
        }
    }
}

#[test]
fn creation_propagates_probe_decode_errors_without_replacing_the_environment() {
    let rig = Rig::new();
    rig.streams(b"\xff", b"");
    let marker = rig.root.join("retain");
    fs::write(&marker, "existing environment").unwrap();
    let output = rig
        .command(&[
            "ida",
            "python",
            "create-environment",
            "--path",
            rig.root.to_str().unwrap(),
            "--python-version",
            "3.12",
            "--no-configure-env-var",
            "--no-reinstall-plugins",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains(DECODE_ERROR));
    assert_eq!(fs::read(marker).unwrap(), b"existing environment");
    assert!(!rig.fixture.calls().contains("pip-run\n"));
}

#[test]
fn path_search_stops_on_decode_errors_before_trying_another_interpreter() {
    let rig = Rig::new();
    rig.streams(b"\xff", b"");
    let bin = rig.fixture.python.parent().unwrap();
    fs::copy(&rig.fixture.python, bin.join("python3.12")).unwrap();
    guard_fixture::executable(
        &bin.join("python3"),
        "#!/bin/sh\nprintf 'unexpected-fallback\\n' >> \"$HY_TEST_EVENTS\"\nprintf '3.12\\n'\n",
    );
    let target = rig.fixture.sandbox.path().join("new-environment");
    let output = rig
        .command(&[
            "ida",
            "python",
            "create-environment",
            "--path",
            target.to_str().unwrap(),
            "--python-version",
            "3.12",
            "--no-configure-env-var",
            "--no-reinstall-plugins",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains(DECODE_ERROR), "{output:?}");
    assert!(!rig.fixture.calls().contains("unexpected-fallback"));
    assert!(!target.exists());
}

#[test]
fn explain_records_mismatch_decode_errors_after_successful_earlier_observations() {
    let rig = Rig::new();
    rig.streams(b"\xff", b"");
    let output = rig
        .command(&["ida", "python", "explain-environment", "--json"])
        .env_remove("HCLI_CURRENT_IDA_PYTHON_EXE")
        .env_remove("IDAPYTHON_VENV_EXECUTABLE")
        .env("HY_TEST_IDA_PROBE", rig.probe().to_string())
        .env("HY_TEST_GOOD_CALLS", "2")
        .output()
        .unwrap();
    assert_success(&output);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["python_version"]["probed_version"], "3.12");
    assert_eq!(report["python_version_mismatches"], serde_json::json!([]));
    assert_eq!(
        report["python_version_mismatch_error"],
        format!("UnicodeDecodeError: {DECODE_ERROR}")
    );
    assert_eq!(rig.fixture.calls().matches("version:").count(), 3);
}

#[test]
fn virtualenv_decode_errors_do_not_fall_back_to_configuration_versions() {
    for doctor in [false, true] {
        let rig = Rig::new();
        let mut command = if doctor {
            let recommended = rig.fixture.sandbox.path().join("idausr/venv");
            fs::create_dir_all(&recommended).unwrap();
            fs::write(recommended.join("pyvenv.cfg"), "version = 3.13\n").unwrap();
            guard_fixture::executable(
                &recommended.join("bin/python"),
                "#!/bin/sh\nprintf '\\377'\n",
            );
            rig.command(&["ida", "python", "doctor", "--json"])
        } else {
            rig.streams(b"\xff", b"");
            let mut command = rig.command(&["ida", "python", "explain-environment", "--json"]);
            command
                .env_remove("HCLI_CURRENT_IDA_PYTHON_EXE")
                .env("HY_TEST_IDA_PROBE", rig.probe().to_string());
            command
        };
        let output = command.output().unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(DECODE_ERROR), "{output:?}");
        assert!(output.stdout.is_empty());
    }
}
