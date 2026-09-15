//! Configuration runs only against an owned home and a fake launchctl command.
#![cfg(target_os = "macos")]

#[path = "create_environment/fixture.rs"]
#[allow(dead_code)]
mod fixture;
mod support;

use std::fs;
use std::path::PathBuf;

use serde_json::Value;

use fixture::Rig;
use support::assert_success;

const PLIST: &str = "Library/LaunchAgents/com.hex-rays.idapython-venv-executable.plist";

fn rig() -> Rig {
    let rig = Rig::new(false);
    rig.existing("python");
    fixture::script(
        &rig.tools.join("launchctl"),
        r#"#!/bin/sh
printf 'launchctl|%s\n' "$*" >> "$HY_TEST_EVENTS"
if [ "${HY_TEST_LAUNCHCTL_STATUS:-0}" != 0 ]; then echo 'fixture command failure' >&2; fi
exit "${HY_TEST_LAUNCHCTL_STATUS:-0}"
"#,
    );
    rig
}

fn paths(rig: &Rig) -> (PathBuf, PathBuf) {
    (rig.sandbox.path().join(PLIST), rig.sandbox.path().join(".zprofile"))
}

fn report(rig: &Rig) -> Value {
    let output = rig.command(true, true).env("SHELL", "/bin/zsh").output().unwrap();
    assert_success(&output);
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn configuration_updates_stale_assignments_and_skips_unchanged_files() {
    let rig = rig();
    let (plist, profile) = paths(&rig);
    fs::write(
        &profile,
        concat!(
            "# before\nexport IDAPYTHON_VENV_EXECUTABLE=old\n",
            "# middle\nexport IDAPYTHON_VENV_EXECUTABLE=older\n# after\n",
        ),
    )
    .unwrap();
    let first = report(&rig);
    assert_eq!(first["configured"], true);
    assert_eq!(first["configured_via"], "macos-launchagent, macos-launchctl-setenv, shell-profile");
    let expected = format!(
        "# before\nexport IDAPYTHON_VENV_EXECUTABLE=\"{}/bin/python\"\n# middle\n# after\n",
        rig.target.display()
    );
    assert_eq!(fs::read_to_string(&profile).unwrap(), expected);
    assert!(fs::read_to_string(&plist).unwrap().contains(rig.target.to_str().unwrap()));
    let times = (
        fs::metadata(&plist).unwrap().modified().unwrap(),
        fs::metadata(&profile).unwrap().modified().unwrap(),
    );
    let second = report(&rig);
    assert_eq!(second["configured"], true);
    assert_eq!(second["configured_via"], "macos-launchctl-setenv");
    assert_eq!(
        (
            fs::metadata(&plist).unwrap().modified().unwrap(),
            fs::metadata(&profile).unwrap().modified().unwrap()
        ),
        times
    );
    assert_eq!(rig.calls().matches("launchctl|").count(), 2);
}

#[test]
fn command_failure_retains_later_file_changes_and_a_successful_creation_report() {
    let rig = rig();
    let output = rig
        .command(true, true)
        .env("SHELL", "/bin/zsh")
        .env("HY_TEST_LAUNCHCTL_STATUS", "7")
        .output()
        .unwrap();
    assert_success(&output);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["configured"], false);
    assert_eq!(report["configured_via"], Value::Null);
    let (plist, profile) = paths(&rig);
    assert!(plist.is_file() && profile.is_file());
    let text = String::from_utf8_lossy(&output.stderr);
    assert!(text.contains("failed (exit 7): fixture command failure"), "{text}");
    assert!(text.contains("Manual instructions:"), "{text}");
    assert!(!text.contains("set and verified"), "{text}");
}

#[test]
fn file_failures_abort_at_the_failing_step_and_preserve_earlier_effects() {
    for fail_profile in [false, true] {
        let rig = rig();
        let (plist, profile) = paths(&rig);
        let failing = if fail_profile {
            &profile
        } else {
            &plist
        };
        fs::create_dir_all(failing).unwrap();
        fs::write(failing.join("retain"), b"retain").unwrap();
        let output = rig.command(true, true).env("SHELL", "/bin/zsh").output().unwrap();
        assert!(!output.status.success());
        assert_eq!(fs::read(failing.join("retain")).unwrap(), b"retain");
        assert_eq!(rig.calls().matches("launchctl|").count(), usize::from(fail_profile));
        if fail_profile {
            assert!(plist.is_file());
        } else {
            assert!(!profile.exists());
        }
    }
}

#[test]
fn an_unknown_shell_still_configures_the_macos_desktop_session() {
    for shell in [None, Some(""), Some("/bin/notbash")] {
        let rig = rig();
        let mut command = rig.command(true, true);
        if let Some(shell) = shell {
            command.env("SHELL", shell);
        } else {
            command.env_remove("SHELL");
        }
        let output = command.output().unwrap();
        assert_success(&output);
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["configured"], true);
        assert_eq!(report["configured_via"], "macos-launchagent, macos-launchctl-setenv");
        assert!(rig.sandbox.path().join(PLIST).is_file());
        assert!(!rig.sandbox.path().join(".bash_profile").exists());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("HCLI does not know how to configure")
        );
    }
}

#[test]
fn declining_the_displayed_plan_leaves_all_configuration_files_untouched() {
    let rig = rig();
    let (plist, profile) = paths(&rig);
    fs::write(&profile, b"# retain\n").unwrap();
    let mut command = rig.command(false, true);
    command.env("SHELL", "/bin/zsh");
    let mut terminal = support::terminal::Terminal::start(command);
    terminal.wait_for("1. Create LaunchAgent");
    terminal.wait_for("2. Apply IDAPYTHON_VENV_EXECUTABLE");
    terminal.wait_for("3. Add export to");
    terminal.wait_for("Apply these changes?");
    terminal.send("n\n");
    let (status, output) = terminal.finish();
    assert!(status.success(), "{output}");
    assert!(output.contains("Configure IDAPYTHON_VENV_EXECUTABLE for your system:"));
    assert!(!plist.exists());
    assert_eq!(fs::read(profile).unwrap(), b"# retain\n");
    assert!(!rig.calls().contains("launchctl|"));
}
