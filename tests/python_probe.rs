//! IDA startup context, fallback boundaries and process environment contracts.
#![cfg(unix)]

use std::fs;
use std::process::Stdio;

#[path = "python_probe/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "python_environment_guard/fixture.rs"]
mod guard_fixture;
mod support;

use fixture::Rig;
use support::assert_success;

#[test]
fn real_user_startup_precedes_isolated_retry_with_minimal_preserved_files() {
    for mode in ["success", "fallback-missing", "fallback-log"] {
        let rig = Rig::new(true);
        let output = rig.command(mode).env("IDA_IS_INTERACTIVE", "parent-value").output().unwrap();
        assert_success(&output);
        let fallback = mode != "success";
        assert_eq!(
            rig.count(),
            if fallback {
                2
            } else {
                1
            }
        );
        let first = rig.environment(1);
        assert_eq!(first["IDAUSR"], rig.idausr.to_str().unwrap());
        assert_eq!(first["IDA_IS_INTERACTIVE"], "1");
        assert!(rig.snapshots.join("idausr-1/plugins/example.py").exists());
        if fallback {
            let second = rig.environment(2);
            assert_ne!(second["IDAUSR"], first["IDAUSR"]);
            assert_eq!(second["IDA_IS_INTERACTIVE"], "parent-value");
            assert!(!std::path::Path::new(&second["IDAUSR"]).exists());
            for relative in
                ["ida.reg", "cfg/idapython.cfg", "idapythonrc.py", "license.hexlic", ".hexlic"]
            {
                assert_eq!(
                    fs::read(rig.snapshots.join("idausr-2").join(relative)).unwrap(),
                    relative.as_bytes()
                );
            }
            assert!(!rig.snapshots.join("idausr-2/plugins").exists());
            assert!(!rig.snapshots.join("idausr-2/unrelated.dat").exists());
        }
        assert_eq!(
            fs::read_to_string(rig.idausr.join("plugins/example.py")).unwrap(),
            "plugins/example.py"
        );
    }
}

#[test]
fn stdout_and_exit_status_do_not_replace_log_results_or_change_retry_boundaries() {
    for user_state in ["absent", "file", "directory"] {
        for mode in ["success", "missing", "malformed", "invalid-model"] {
            for status in [0, 17] {
                let user_directory = user_state == "directory";
                let rig = Rig::new(user_directory);
                if user_state == "file" {
                    fs::write(&rig.idausr, "preserved").unwrap();
                }
                let output = rig
                    .command(mode)
                    .env("HY_TEST_IDAT_STATUS", status.to_string())
                    .output()
                    .unwrap();
                assert_eq!(
                    output.status.success(),
                    mode == "success",
                    "{mode}/{status}: {output:?}"
                );
                assert_eq!(
                    rig.count(),
                    if user_directory && mode == "missing" {
                        2
                    } else {
                        1
                    }
                );
                assert!(
                    !String::from_utf8_lossy(&output.stdout).contains("stdout_is_not_the_result")
                );
                if user_state == "absent" {
                    assert!(!rig.idausr.exists());
                } else if user_state == "file" {
                    assert_eq!(fs::read_to_string(&rig.idausr).unwrap(), "preserved");
                }
            }
        }
    }
}

#[test]
fn batch_environment_preserves_startup_variables_cwd_and_stdin() {
    for activated in [false, true] {
        let rig = Rig::new(true);
        let input = rig.fixture.sandbox.path().join("input");
        fs::write(&input, "first line\nsecond line\n").unwrap();
        let mut command = rig.command("fallback-log");
        command
            .env("PYTHONHOME", "/parent/home")
            .env("PYTHONPATH", "/parent/modules")
            .env("PYTHONUTF8", "0")
            .env("IDADIR", "/parent/idadir")
            .env("IDAPYTHON_VENV_EXECUTABLE", "missing-interpreter")
            .env("HY_TEST_READ_STDIN", "1")
            .current_dir(rig.fixture.sandbox.path())
            .stdin(Stdio::from(fs::File::open(&input).unwrap()));
        if activated {
            command.env("VIRTUAL_ENV", "/parent/venv");
        }
        assert_success(&command.output().unwrap());
        for attempt in [1, 2] {
            let env = rig.environment(attempt);
            for name in ["PYTHONHOME", "PYTHONPATH", "PATH"] {
                assert!(!env.contains_key(name), "{name}: {env:?}");
            }
            assert_eq!(
                env.get("VIRTUAL_ENV").map(String::as_str),
                activated.then_some("/parent/venv")
            );
            assert_eq!(env["PYTHONUTF8"], "0");
            assert_eq!(env["IDADIR"], "/parent/idadir");
            assert_eq!(env["IDAPYTHON_VENV_EXECUTABLE"], "missing-interpreter");
            assert_eq!(
                fs::read_to_string(rig.snapshots.join(format!("stdin-{attempt}"))).unwrap(),
                if attempt == 1 {
                    "first line\n"
                } else {
                    "second line\n"
                }
            );
            let cwd = fs::read_to_string(rig.snapshots.join(format!("cwd-{attempt}"))).unwrap();
            assert_eq!(
                fs::canonicalize(cwd.trim()).unwrap(),
                fs::canonicalize(rig.fixture.sandbox.path()).unwrap()
            );
            let args = fs::read_to_string(rig.snapshots.join(format!("args-{attempt}"))).unwrap();
            assert!(args.starts_with("-a\n-A\n-c\n-t\n-L"), "{args}");
            for argument in args
                .lines()
                .filter(|argument| argument.starts_with("-L") || argument.starts_with("-S"))
            {
                assert!(!std::path::Path::new(&argument[2..]).exists());
            }
            let script =
                fs::read_to_string(rig.snapshots.join(format!("script-{attempt}.py"))).unwrap();
            assert_eq!(script.trim(), include_str!("../src/ida/python/probe/source.py").trim());
        }
    }
}
