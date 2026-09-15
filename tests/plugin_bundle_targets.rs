//! CLI target selection uses owned archives and version-only interpreter fixtures.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

mod support;

use support::{Sandbox, archive, assert_success};

struct Fixture {
    sandbox: Sandbox,
    package: PathBuf,
    output: PathBuf,
    python: PathBuf,
    calls: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let sandbox = Sandbox::new();
        let package = sandbox.path().join("plugin.zip");
        archive(&package, "1", &[]);
        let output = sandbox.path().join("bundle.zip");
        let python = sandbox.path().join("python");
        fs::write(&python, include_str!("plugin_bundle_targets/python.sh")).unwrap();
        fs::set_permissions(&python, fs::Permissions::from_mode(0o755)).unwrap();
        let calls = sandbox.path().join("calls");
        Self {
            sandbox,
            package,
            output,
            python,
            calls,
        }
    }

    fn command(&self, options: &[&str]) -> Command {
        let mut args = vec!["plugin", "bundle", "create", "--path", self.output.to_str().unwrap()];
        args.extend(options);
        args.push(self.package.to_str().unwrap());
        let mut command = self.sandbox.command(&args);
        command
            .env("HCLI_CURRENT_IDA_PYTHON_EXE", &self.python)
            .env("HCLI_CURRENT_IDA_PLATFORM", "windows-x86_64")
            .env("HY_TEST_CALLS", &self.calls);
        command
    }

    fn targets(&self) -> Vec<Value> {
        let mut zip = zip::ZipArchive::new(fs::File::open(&self.output).unwrap()).unwrap();
        let manifest: Value =
            serde_json::from_reader(zip.by_name("plugin-bundle.json").unwrap()).unwrap();
        manifest["targetPlatformTags"].as_array().unwrap().clone()
    }
}

#[test]
fn selected_versions_preserve_unicode_and_newlines_while_alias_options_deduplicate() {
    let cases = [
        (
            vec![
                "--platform",
                "windows",
                "--platform",
                "WIN",
                "--python",
                "3.12",
                "--python",
                "3.12",
            ],
            "3.12",
        ),
        (vec!["--platform", "\u{1c}WIN\u{1f}", "--python", "٣.١٢"], "٣.١٢"),
        (vec!["--platform", "windows", "--python", "3.12\n"], "3.12\n"),
        (vec!["--target", "windows-x86_64-cp３１２\n"], "３.１２"),
    ];
    for (options, version) in cases {
        let fixture = Fixture::new();
        assert_success(&fixture.command(&options).output().unwrap());
        let targets = fixture.targets();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0]["pythonVersion"], version);
        assert_eq!(targets[0]["id"], format!("windows-x86_64-cp{}", version.replace('.', "")));
        assert!(!fixture.calls.exists());
    }
}

#[test]
fn explicit_targets_keep_duplicates_and_reject_aliases_at_tag_generation() {
    let fixture = Fixture::new();
    assert_success(
        &fixture
            .command(&["--target", "windows-x86_64-cp312", "--target", "windows-x86_64-cp312"])
            .output()
            .unwrap(),
    );
    let targets = fixture.targets();
    assert_eq!(targets.len(), 2);
    assert_eq!(targets[0], targets[1]);
    // The source selector preserves duplicates; its manifest reader rejects them.
    let info = fixture.sandbox.run(&["plugin", "bundle", "info", fixture.output.to_str().unwrap()]);
    assert!(!info.status.success());
    for options in
        [vec!["--target", "windows-cp312"], vec!["--platform", "current", "--python", "3.12"]]
    {
        let fixture = Fixture::new();
        fs::write(&fixture.output, "original bundle").unwrap();
        let output =
            fixture.command(&options).env("HCLI_CURRENT_IDA_PLATFORM", "windows").output().unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("unsupported platform: windows (available:")
        );
        assert_eq!(fs::read(&fixture.output).unwrap(), b"original bundle");
    }
}

#[test]
fn current_python_uses_repeated_version_only_observations() {
    let fixture = Fixture::new();
    let output = fixture
        .command(&["--platform", "current", "--python", "current", "--python", "current"])
        .env("HY_TEST_VERSION", " \r\n3.13\r\n ")
        .env("PYTHONHOME", "fixture-home")
        .env("VIRTUAL_ENV", "fixture-venv")
        .env("PYTHONUTF8", "0")
        .output()
        .unwrap();
    assert_success(&output);
    assert_eq!(fixture.targets()[0]["pythonVersion"], "3.13");
    let calls = fs::read_to_string(&fixture.calls).unwrap();
    assert_eq!(calls.lines().collect::<Vec<_>>(), ["version:fixture-home|fixture-venv|0"; 2]);
}

#[test]
fn current_python_probe_failures_keep_source_failure_boundary() {
    for (version, status, expected) in [
        ("", "0", "failed to probe the version of IDA's Python interpreter:"),
        ("3.12", "7", "failed to probe the version of IDA's Python interpreter:"),
        ("nonsense", "0", "invalid python version: 'nonsense'"),
        ("3.9", "0", "python 3.9 is below minimum 3.10"),
    ] {
        let fixture = Fixture::new();
        let output = fixture
            .command(&["--platform", "windows", "--python", "current"])
            .env("HY_TEST_VERSION", version)
            .env("HY_TEST_STATUS", status)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(expected), "{output:?}");
        assert!(!fixture.output.exists());
    }
}

#[test]
fn missing_selection_options_fail_before_interpreter_observation() {
    for (options, expected) in [
        (vec![], "--platform is required"),
        (vec!["--platform", "windows"], "--python is required"),
        (
            vec!["--target", "windows-x86_64-cp312", "--python", "current"],
            "--target cannot be combined",
        ),
    ] {
        let fixture = Fixture::new();
        let output = fixture.command(&options).output().unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(expected));
        assert!(!fixture.calls.exists());
    }
}
