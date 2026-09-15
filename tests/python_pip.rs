//! Pip subprocess contracts use shell stand-ins, never an installed package manager.
#![cfg(unix)]

use std::fs;
use std::process::{Command, Stdio};

#[allow(dead_code)]
#[path = "python_environment_guard/fixture.rs"]
mod fixture;
mod support;

use fixture::Fixture;
use support::{archive_with_dependencies, assert_success};

fn setup() -> (Fixture, Command) {
    let fixture = Fixture::new(true);
    fixture::executable(&fixture.python, include_str!("python_pip/python.sh"));
    let package = fixture.sandbox.path().join("candidate.zip");
    archive_with_dependencies(&package, "1.0", &[], &["fixture[extra]>=1", "--pre"]);
    let command = fixture.command(&[
        "plugin",
        "--no-python-environment-check",
        "install",
        package.to_str().unwrap(),
    ]);
    (fixture, command)
}

#[test]
fn pip_inherits_environment_and_stdin_and_keeps_success_output_captured() {
    let (fixture, mut command) = setup();
    let input = fixture.sandbox.path().join("stdin");
    fs::write(&input, "for-resolution\nfor-installation\n").unwrap();
    command
        .stdin(Stdio::from(fs::File::open(input).unwrap()))
        .env("HY_TEST_READ_STDIN", "1")
        .env("PYTHONHOME", "/fixture/base python")
        .env("VIRTUAL_ENV", "/fixture/parent environment")
        .env("PYTHONUTF8", "0")
        .env("PATH", "/fixture/bin:/other path/bin")
        .env("HY_TEST_PIP_STDOUT", "captured pip stdout")
        .env("HY_TEST_PIP_STDERR", "externally-managed-environment");
    let output = command.output().unwrap();
    assert_success(&output);
    let calls = fixture.calls();
    let environments: Vec<_> =
        calls.lines().filter(|line| line.starts_with("environment:")).collect();
    assert_eq!(
        environments,
        vec![
            "environment:/fixture/base python|/fixture/parent environment|0|/fixture/bin:/other path/bin";
            2
        ]
    );
    assert!(calls.contains("stdin:for-resolution\n") && calls.contains("stdin:for-installation\n"));
    assert!(calls.contains("arg:fixture[extra]>=1\narg:--pre\n"));
    assert!(!calls.contains("arg:--\n"));
    assert!(!calls.contains("arg:--disable-pip-version-check\n"));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("captured pip stdout"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("externally-managed-environment"));
}

#[test]
fn pip_find_links_expand_home_and_preserve_url_and_relative_path_semantics() {
    let fixture = Fixture::new(true);
    fixture::executable(&fixture.python, include_str!("python_pip/python.sh"));
    let package = fixture.sandbox.path().join("candidate.zip");
    archive_with_dependencies(&package, "1.0", &[], &["fixture"]);
    let links = [
        "",
        ".//wheels/",
        "~/cache/./wheels/../",
        "//shared///wheels/",
        "https://host/a//./b",
        "odd://~//",
        "~unknown://keep",
    ];
    let mut args = vec!["plugin", "--no-python-environment-check", "--offline"];
    for link in links {
        args.extend(["--pip-find-links", link]);
    }
    args.extend(["install", package.to_str().unwrap()]);
    assert_success(&fixture.command(&args).output().unwrap());
    let calls = fixture.calls();
    let lines: Vec<_> = calls.lines().collect();
    let actual: Vec<_> = lines
        .windows(2)
        .filter(|lines| lines[0] == "arg:--find-links")
        .map(|lines| lines[1].strip_prefix("arg:").unwrap())
        .collect();
    let home_link = format!("{}/cache/wheels/..", fixture.sandbox.path().display());
    let expected = [
        ".",
        "wheels",
        &home_link,
        "//shared/wheels",
        "https://host/a//./b",
        "odd://~//",
        "~unknown://keep",
    ];
    assert_eq!(actual, expected.repeat(2));
}

#[test]
fn repository_free_commands_still_expand_local_pip_links() {
    let sandbox = support::Sandbox::new();
    let missing = "~hy-no-such-user-6a08e7c1/wheels";
    let output = sandbox.run(&["plugin", "--pip-find-links", missing, "schema"]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Could not determine home directory.")
    );
    assert!(output.stdout.is_empty());
    for home in ["", "relative/home", "~unresolved"] {
        let output = sandbox
            .command(&["plugin", "--pip-find-links", "~/wheels", "schema"])
            .env("HOME", home)
            .output()
            .unwrap();
        assert_eq!(output.status.success(), home != "~unresolved", "{home}: {output:?}");
    }
    assert_success(&sandbox.run(&["plugin", "--pip-find-links", "~unknown://keep", "schema"]));
}

#[test]
fn pip_failures_preserve_stream_order_and_recognize_known_errors_at_both_stages() {
    for phase in ["resolution", "installation"] {
        for kind in ["ordinary", "managed", "old-pip", "both", "empty"] {
            let (fixture, mut command) = setup();
            let (stdout, stderr) = match kind {
                "ordinary" => ("  diagnostic stdout\n", "\n diagnostic stderr  "),
                "managed" => ("externally-managed-environment", ""),
                "old-pip" => ("", "no such option: --dry-run"),
                "both" => ("no such option: --dry-run", "externally-managed-environment"),
                _ => (" \n", "\t"),
            };
            let output = command
                .env("HY_TEST_FAIL_PHASE", phase)
                .env("HY_TEST_PIP_STDOUT", stdout)
                .env("HY_TEST_PIP_STDERR", stderr)
                .output()
                .unwrap();
            assert!(!output.status.success());
            let error = String::from_utf8_lossy(&output.stderr);
            assert_eq!(
                error.contains("Cannot install required Python dependencies:"),
                phase == "resolution",
                "{error}"
            );
            match kind {
                "ordinary" => {
                    assert!(error.contains("diagnostic stdout\ndiagnostic stderr"), "{error}")
                }
                "managed" | "both" => {
                    assert!(error.contains("is an externally managed Python (PEP 668)"), "{error}");
                    assert!(!error.contains("pip does not support --dry-run"));
                }
                "old-pip" => assert!(
                    error.contains("pip does not support --dry-run (requires pip 22.2 or later)"),
                    "{error}"
                ),
                _ => assert!(!error.contains(". Reason:"), "{error}"),
            }
            assert_eq!(
                fixture.calls().matches("phase:").count(),
                if phase == "resolution" {
                    1
                } else {
                    2
                }
            );
            assert!(!fixture.sandbox.path().join("idausr/plugins/example").exists());
        }
    }
}
