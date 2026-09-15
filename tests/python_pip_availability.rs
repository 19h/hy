//! Pip import checks inherit process state and interpret only return status.
#![cfg(unix)]

use std::fs;
use std::process::Stdio;

#[allow(dead_code)]
#[path = "python_environment_guard/fixture.rs"]
mod fixture;
mod support;

use fixture::Fixture;
use support::{archive_with_dependencies, assert_success};

fn setup() -> Fixture {
    let fixture = Fixture::new(true);
    fixture::executable(&fixture.python, include_str!("python_pip_availability/python.sh"));
    fixture
}

#[test]
fn pip_import_and_version_share_inherited_stdin_with_the_requested_child() {
    let fixture = setup();
    let input = fixture.sandbox.path().join("stdin");
    fs::write(&input, "pip-line\nversion-line\nchild-line\n").unwrap();
    let output = fixture
        .command(&["ida", "python", "exec", "-c", "fixture"])
        .stdin(Stdio::from(fs::File::open(input).unwrap()))
        .env("HY_TEST_READ_INPUT", "1")
        .env("PYTHONHOME", "fixture-home")
        .env("VIRTUAL_ENV", "fixture-venv")
        .env("PYTHONUTF8", "0")
        .output()
        .unwrap();
    assert_success(&output);
    let calls = fixture.calls();
    assert!(calls.contains("pip-input:pip-line\nversion-input:version-line\n"), "{calls}");
    assert!(calls.contains("pip-env:fixture-home|fixture-venv|0\n"), "{calls}");
    assert_eq!(output.stdout, b"child-input:child-line\n");
}

#[test]
fn malformed_import_output_is_ignored_at_both_installation_checks() {
    for skip in [false, true] {
        for status in [0, 7] {
            let fixture = setup();
            let package = fixture.sandbox.path().join("plugin.zip");
            archive_with_dependencies(&package, "1", &[], &["fixture"]);
            let mut args = vec!["plugin"];
            if skip {
                args.push("--no-python-environment-check");
            }
            args.extend(["install", package.to_str().unwrap()]);
            let output = fixture
                .command(&args)
                .env("HY_TEST_PIP_STATUS", status.to_string())
                .output()
                .unwrap();
            assert_eq!(output.status.success(), status == 0, "{output:?}");
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(!stderr.contains("codec can't decode"));
            assert!(!output.stdout.contains(&0xff) && !output.stderr.contains(&0xff));
            if status == 0 {
                assert_eq!(
                    fixture.calls().matches("pip-env:").count(),
                    if skip {
                        1
                    } else {
                        2
                    }
                );
                assert_eq!(fixture.calls().matches("pip-run\n").count(), 2);
            } else {
                assert!(!fixture.calls().contains("pip-run\n"));
            }
        }
    }
}
