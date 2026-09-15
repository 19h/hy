//! Console-script routing, probe failures and child environment contracts.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

#[allow(dead_code)]
#[path = "python_environment_guard/fixture.rs"]
mod fixture;
mod support;

use fixture::{Fixture, executable};
use support::assert_success;

fn command(fixture: &Fixture, leaf: &str, document: &Value) -> Command {
    executable(&fixture.python, include_str!("python_scripts/python.sh"));
    let mut command =
        fixture.command(&["ida", "python", "--no-python-environment-check", leaf, "fixture-tool"]);
    command.env("HY_TEST_LOOKUP_OUTPUT", format!("startup noise\n__hcli__:{document}\n"));
    command
}

fn document(path: Option<&std::path::Path>, entry_point: bool) -> Value {
    json!({
        "name": "fixture-tool", "path": path, "scripts_dirs": ["/first", "/second"],
        "entry_point": entry_point.then(|| json!({
            "name": "fixture-tool", "value": "fixture:main", "group": "console_scripts",
            "distribution": "fixture-package", "version": "1.2",
        })),
    })
}

#[test]
fn wrapper_execution_uses_permissions_instead_of_filename_extension() {
    for name in ["fixture-tool", "fixture-tool.py"] {
        for executable_bit in [false, true] {
            let fixture = Fixture::new(true);
            let wrapper = fixture.python.parent().unwrap().join(name);
            executable(&wrapper, "#!/bin/sh\nprintf 'wrapper\n'\nprintf '<%s>\n' \"$@\"\nexit 7\n");
            if !executable_bit {
                fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o644)).unwrap();
            }
            let output = command(&fixture, "run-script", &document(Some(&wrapper), true))
                .args(["argument with spaces", "--help", ""])
                .env("HY_TEST_CHILD_STATUS", "7")
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(7), "{output:?}");
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("<argument with spaces>\n<--help>\n<>\n"), "{stdout}");
            assert_eq!(stdout.contains("wrapper\n"), executable_bit, "{stdout}");
            assert_eq!(fixture.calls().contains("python\n"), !executable_bit);
            assert!(!fixture.calls().contains("entry-point"));
        }
    }
}

#[test]
fn missing_wrapper_uses_entry_point_only_for_execution() {
    for leaf in ["find-script", "run-script"] {
        for entry in [false, true] {
            let fixture = Fixture::new(false);
            let output = command(&fixture, leaf, &document(None, entry)).output().unwrap();
            if leaf == "run-script" && entry {
                assert_success(&output);
                assert!(fixture.calls().contains("entry-point"));
                assert!(String::from_utf8_lossy(&output.stdout).starts_with("<fixture-tool>\n"));
            } else {
                assert_eq!(output.status.code(), Some(1));
                assert!(!fixture.calls().contains("entry-point"));
                let stderr = String::from_utf8_lossy(&output.stderr);
                assert!(stderr.contains(if entry {
                    "declared by fixture-package 1.2 (fixture:main), but no program for it was installed."
                } else {
                    "script 'fixture-tool' is not installed."
                }), "{stderr}");
                assert!(stderr.contains("  /first\n  /second"), "{stderr}");
            }
        }
    }
}

#[test]
fn probe_failure_precedes_valid_output_and_framing_ignores_unprefixed_noise() {
    for (stdout, stderr, status, expected) in [
        (
            "__hcli__:{\"name\":\"fixture\",\"path\":\"/found\"}\n",
            "  failed\n",
            9,
            "exited with status 9: failed",
        ),
        ("noise __hcli__:{}\n", "", 0, "no result from"),
        ("__hcli__:invalid\n__hcli__:{}\n", "", 0, "JSON error"),
        ("startup\r\n__hcli__:{\"name\":\"fixture\",\"path\":\"/found\"}\r\n", "", 0, "/found"),
    ] {
        let fixture = Fixture::new(false);
        let output = command(&fixture, "find-script", &document(None, false))
            .env("HY_TEST_LOOKUP_OUTPUT", stdout)
            .env("HY_TEST_LOOKUP_ERROR", stderr)
            .env("HY_TEST_LOOKUP_STATUS", status.to_string())
            .output()
            .unwrap();
        let stream = if expected == "/found" {
            &output.stdout
        } else {
            &output.stderr
        };
        assert!(String::from_utf8_lossy(stream).contains(expected), "{output:?}");
        assert_eq!(output.status.success(), expected == "/found");
        assert_eq!(fixture.calls(), "lookup\n");
    }
}

#[test]
fn execution_rebuilds_only_the_selected_python_environment_and_preserves_stdin() {
    for venv in [false, true] {
        for mode in ["exec", "entry-point", "wrapper"] {
            for path_mode in ["unset", "empty", "populated"] {
                let fixture = Fixture::new(venv);
                let input = fixture.sandbox.path().join("stdin");
                fs::write(&input, "input with spaces\n").unwrap();
                let wrapper = fixture.sandbox.path().join("elsewhere/wrapper.py");
                executable(&wrapper, include_str!("python_scripts/python.sh"));
                let directory = fixture.python.parent().unwrap();
                let populated = format!(":{}::/parent", directory.display());
                let inherited = if path_mode == "populated" {
                    &populated
                } else {
                    ""
                };
                let leaf = if mode == "exec" {
                    "exec"
                } else {
                    "run-script"
                };
                let info = document((mode == "wrapper").then_some(wrapper.as_path()), true);
                let mut command = command(&fixture, leaf, &info);
                command
                    .env("PYTHONHOME", "/parent/home")
                    .env("VIRTUAL_ENV", "/parent/venv")
                    .env("PYTHONUTF8", "0")
                    .env("HY_TEST_READ_STDIN", "1")
                    .stdin(Stdio::from(fs::File::open(input).unwrap()));
                if path_mode == "unset" {
                    command.env_remove("PATH");
                } else {
                    command.env("PATH", inherited);
                }
                let output = command.output().unwrap();
                assert_success(&output);
                let root = directory.parent().unwrap();
                let selected = if venv {
                    root.to_str().unwrap()
                } else {
                    "unset"
                };
                let expected_path = if inherited.is_empty() {
                    directory.display().to_string()
                } else {
                    format!("{}:{inherited}", directory.display())
                };
                let stdout = String::from_utf8_lossy(&output.stdout);
                assert!(
                    stdout.contains(&format!("environment:unset|{selected}|0|{expected_path}\n")),
                    "{mode}/{path_mode}: {stdout}"
                );
                assert!(stdout.contains("stdin:input with spaces\n"), "{stdout}");
            }
        }
    }
}

#[test]
fn child_signal_return_codes_follow_subprocess_returncode() {
    for signal in ["TERM", "HUP"] {
        let fixture = Fixture::new(false);
        let output = command(&fixture, "exec", &document(None, false))
            .env("HY_TEST_SIGNAL", signal)
            .output()
            .unwrap();
        let number = if signal == "TERM" {
            libc::SIGTERM
        } else {
            libc::SIGHUP
        };
        assert_eq!(output.status.code(), Some(256 - number));
        assert!(output.stderr.is_empty(), "{output:?}");
    }
}

#[test]
fn real_interpreter_resolves_record_wrappers_user_scripts_and_entry_points() {
    let Some(python) = std::env::var_os("HY_TEST_VENV_ORACLE_PYTHON") else {
        return;
    };
    let name = "__hy_parity_script_fixture__";
    for mode in ["record-executable", "record-python", "entry-point", "user-script"] {
        let sandbox = support::Sandbox::new();
        let site = sandbox.path().join("site-packages");
        let metadata = site.join("hy_parity_fixture-1.2.dist-info");
        fs::create_dir_all(&metadata).unwrap();
        fs::write(
            metadata.join("METADATA"),
            "Metadata-Version: 2.1\nName: hy-parity-fixture\nVersion: 1.2\n",
        )
        .unwrap();
        fs::write(site.join("hy_parity_fixture.py"),
            "import json, sys\ndef main():\n    print(json.dumps(sys.argv, ensure_ascii=False))\n    return 7\n").unwrap();
        if mode != "user-script" {
            fs::write(
                metadata.join("entry_points.txt"),
                format!("[console_scripts]\n{name} = hy_parity_fixture:main\n"),
            )
            .unwrap();
        }
        let user_base = sandbox.path().join("user");
        let wrapper = if mode == "user-script" {
            user_base.join("bin").join(name)
        } else {
            sandbox.path().join("wrappers").join(name)
        };
        let wrapper_exists = mode != "entry-point";
        if wrapper_exists {
            if mode == "record-python" {
                fs::create_dir_all(wrapper.parent().unwrap()).unwrap();
                fs::write(&wrapper, "import json, sys\nprint(json.dumps(sys.argv, ensure_ascii=False))\nsys.exit(7)\n").unwrap();
            } else {
                executable(
                    &wrapper,
                    "#!/bin/sh\nprintf 'wrapper\n'\nprintf '<%s>\n' \"$@\"\nexit 7\n",
                );
            }
        }
        // The first record entry is missing; the second resolves through a symlink.
        if mode.starts_with("record-") {
            let linked = sandbox.path().join("linked");
            fs::create_dir(&linked).unwrap();
            std::os::unix::fs::symlink(&wrapper, linked.join(name)).unwrap();
            fs::write(
                metadata.join("RECORD"),
                format!(
                    "{}/missing/{name},,\n{}/{name},,\n",
                    sandbox.path().display(),
                    linked.display(),
                ),
            )
            .unwrap();
        }
        for leaf in ["find-script", "run-script"] {
            let mut command =
                sandbox.command(&["ida", "python", "--no-python-environment-check", leaf, name]);
            command
                .env("HCLI_CURRENT_IDA_PYTHON_EXE", &python)
                .env("PYTHONPATH", &site)
                .env("PYTHONUSERBASE", &user_base)
                .env("PYTHONDONTWRITEBYTECODE", "1");
            if leaf == "run-script" {
                command.args(["argument with spaces", "--help", "λ"]);
            }
            let output = command.output().unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            if leaf == "find-script" {
                assert_eq!(output.status.success(), wrapper_exists, "{mode}: {output:?}");
                if wrapper_exists {
                    let resolved = fs::canonicalize(&wrapper).unwrap();
                    // RECORD paths are realpaths; fallback paths retain their spelling.
                    let expected = if mode == "user-script" {
                        &wrapper
                    } else {
                        &resolved
                    };
                    assert_eq!(stdout.trim_end(), expected.to_str().unwrap(), "{mode}");
                } else {
                    assert!(
                        String::from_utf8_lossy(&output.stderr).contains("hy-parity-fixture 1.2")
                    );
                }
            } else {
                assert_eq!(output.status.code(), Some(7), "{mode}: {output:?}");
                if matches!(mode, "entry-point" | "record-python") {
                    let argv: Vec<String> = serde_json::from_slice(&output.stdout).unwrap();
                    let first = if mode == "entry-point" {
                        name.to_owned()
                    } else {
                        fs::canonicalize(&wrapper).unwrap().to_str().unwrap().to_owned()
                    };
                    assert_eq!(
                        argv,
                        [first, "argument with spaces".into(), "--help".into(), "λ".into()]
                    );
                } else {
                    assert_eq!(stdout, "wrapper\n<argument with spaces>\n<--help>\n<λ>\n");
                }
            }
        }
        assert!(!site.join("__pycache__").exists());
    }
}
