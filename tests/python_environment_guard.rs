//! Environment checks have different blocking policies for installation and execution.
#![cfg(unix)]

use std::fs;

#[path = "python_environment_guard/fixture.rs"]
mod fixture;
mod support;

use fixture::Fixture;
use support::{archive, archive_with_dependencies, assert_success};

#[test]
fn execution_commands_warn_without_blocking_and_honor_the_group_skip_flag() {
    for leaf in ["exec", "run-script", "find-script"] {
        for skip in [false, true] {
            let fixture = Fixture::new(false);
            let mut args = vec!["ida", "python"];
            if skip {
                args.push("--no-python-environment-check");
            }
            args.push(leaf);
            match leaf {
                "exec" => args.extend(["-c", "fixture", "--child-option"]),
                "run-script" => args.extend(["fixture-tool", "--child-option"]),
                _ => args.push("fixture-tool"),
            }
            let output = fixture.command(&args).env("HY_TEST_CHILD_STATUS", "7").output().unwrap();
            assert_eq!(
                output.status.code(),
                Some(if leaf == "find-script" {
                    0
                } else {
                    7
                })
            );
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert_eq!(
                stderr.contains("Error: HCLI cannot install plugin dependencies"),
                !skip,
                "{stderr}"
            );
            assert_eq!(stderr.contains("overrides normal Python detection"), !skip);
            let calls = fixture.calls();
            assert_eq!(calls.contains("pip-check"), !skip);
            assert_eq!(calls.contains("version-check"), !skip);
            assert!(!calls.contains("idat"));
            assert!(String::from_utf8_lossy(&output.stdout).contains(if leaf == "find-script" {
                fixture.script.to_str().unwrap()
            } else if leaf == "exec" {
                "child output"
            } else {
                "script output"
            }));
        }
    }
}

#[test]
fn warning_only_checks_do_not_probe_ida_for_a_configured_environment() {
    let fixture = Fixture::new(true);
    let mut command = fixture.command(&["ida", "python", "exec", "--version"]);
    fixture.configure(&mut command, "3.12");
    let output = command.output().unwrap();
    assert_success(&output);
    assert!(!String::from_utf8_lossy(&output.stderr).contains("version mismatch"));
    assert!(!fixture.calls().contains("idat"));
    assert!(fixture.calls().contains("child|--version"));
}

#[test]
fn group_skip_flag_is_distinct_from_the_identically_named_child_argument() {
    for group_skip in [false, true] {
        let fixture = Fixture::new(false);
        let mut args = if group_skip {
            vec!["ida", "python", "--no-python-environment-check"]
        } else {
            vec!["ida", "python"]
        };
        args.extend(["exec", "--no-python-environment-check", "--child-option"]);
        let output = fixture.command(&args).output().unwrap();
        assert_success(&output);
        assert_eq!(
            String::from_utf8_lossy(&output.stderr).contains("overrides normal Python detection"),
            !group_skip
        );
        assert!(fixture.calls().contains("child|--no-python-environment-check --child-option"));
    }
}

#[test]
fn dependency_checks_reject_errors_before_pip_or_publication_and_allow_warnings() {
    for scenario in ["healthy", "override", "base", "no-pip", "mismatch", "overlay"] {
        for skip in [false, true] {
            let fixture = Fixture::new(scenario != "base");
            let original = fixture.sandbox.path().join("original.zip");
            archive(&original, "1.0", &[("package/sentinel", b"retain")]);
            assert_success(
                &fixture
                    .command(&["plugin", "install", original.to_str().unwrap()])
                    .output()
                    .unwrap(),
            );
            let package = fixture.sandbox.path().join("candidate.zip");
            archive_with_dependencies(&package, "2.0", &[], &["fixture-dependency==1"]);
            if scenario == "overlay" {
                fs::write(
                    fixture.python.parent().unwrap().parent().unwrap().join("pyvenv.cfg"),
                    "extends-environment = fixture\n",
                )
                .unwrap();
            }
            let mut args = vec!["plugin"];
            if skip {
                args.push("--no-python-environment-check");
            }
            args.extend(["install", "-U", package.to_str().unwrap()]);
            let mut command = fixture.command(&args);
            if matches!(scenario, "healthy" | "mismatch") {
                fixture.configure(
                    &mut command,
                    if scenario == "mismatch" {
                        "3.12"
                    } else {
                        "3.13"
                    },
                );
            }
            if scenario == "no-pip" {
                command.env("HY_TEST_PIP_STATUS", "1");
            }
            let output = command.output().unwrap();
            let succeeds =
                scenario != "no-pip" && (skip || matches!(scenario, "healthy" | "override"));
            assert_eq!(output.status.success(), succeeds, "{scenario}, skip={skip}: {output:?}");
            let calls = fixture.calls();
            assert_eq!(
                calls.matches("pip|").count(),
                if succeeds {
                    2
                } else {
                    0
                }
            );
            let installed = fixture.sandbox.path().join("idausr/plugins/example");
            if !succeeds {
                assert_eq!(fs::read(installed.join("sentinel")).unwrap(), b"retain");
                let manifest: serde_json::Value =
                    serde_json::from_slice(&fs::read(installed.join("ida-plugin.json")).unwrap())
                        .unwrap();
                assert_eq!(manifest["plugin"]["version"], "1.0");
            }
            let stderr = String::from_utf8_lossy(&output.stderr);
            if scenario == "no-pip" && skip {
                assert!(stderr.contains("pip is not available in IDA's Python environment at"));
                assert!(!stderr.contains("To skip this check:"));
            }
            if scenario == "mismatch" {
                assert_eq!(calls.contains("idat"), !skip);
                assert_eq!(stderr.contains("Python version mismatch"), !skip);
            }
            if scenario == "override" && !skip {
                assert!(
                    stderr.contains(
                        "Warning: IDA's Python environment is not the recommended setup."
                    )
                );
            }
        }
    }
}

#[test]
fn malformed_neighbor_dependencies_fail_before_interpreter_checks() {
    let fixture = Fixture::new(false);
    let neighbor = fixture.sandbox.path().join("idausr/plugins/neighbor");
    fs::create_dir_all(&neighbor).unwrap();
    let mut metadata = support::identity_manifest("1.0", "https://github.com/example/plugins");
    metadata["plugin"]["name"] = serde_json::json!("neighbor");
    metadata["plugin"]["pythonDependencies"] = serde_json::json!("inline");
    fs::write(neighbor.join("ida-plugin.json"), serde_json::to_vec(&metadata).unwrap()).unwrap();
    fs::write(neighbor.join("plugin.py"), "# /// script\n# dependencies = [\n# ///\n").unwrap();
    let package = fixture.sandbox.path().join("candidate.zip");
    archive_with_dependencies(&package, "1.0", &[], &["dependency"]);
    let output =
        fixture.command(&["plugin", "install", package.to_str().unwrap()]).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid PEP 723 metadata"));
    assert!(fixture.calls().is_empty());
    assert!(!fixture.sandbox.path().join("idausr/plugins/example").exists());
}

#[test]
fn execution_skip_flag_does_not_disable_explicit_doctor_diagnostics() {
    let fixture = Fixture::new(false);
    let regular = fixture.command(&["ida", "python", "doctor", "--json"]).output().unwrap();
    let skipped = fixture
        .command(&["ida", "python", "--no-python-environment-check", "doctor", "--json"])
        .output()
        .unwrap();
    assert_eq!(regular.status.code(), Some(1));
    assert_eq!(skipped.status.code(), Some(1));
    assert_eq!(regular.stdout, skipped.stdout);
    assert_eq!(fixture.calls().matches("pip-check").count(), 2);
}
