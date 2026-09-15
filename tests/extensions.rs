//! Real Python entry-point registration and Click execution. See docs/parity.md.
#![cfg(unix)]

mod support;

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;
use support::terminal::Terminal;
use support::*;

struct RuntimeFixture {
    sandbox: Sandbox,
    python: PathBuf,
    site: PathBuf,
}

impl RuntimeFixture {
    fn new() -> Self {
        let python = std::env::var_os("HY_TEST_EXTENSION_PYTHON").map(PathBuf::from).expect(
            "set HY_TEST_EXTENSION_PYTHON to an interpreter with pinned ida-hcli installed",
        );
        let sandbox = Sandbox::new();
        let site = sandbox.path().join("site");
        let metadata = site.join("hy_extension_fixture-1.2.3.dist-info");
        fs::create_dir_all(&metadata).unwrap();
        fs::write(site.join("hy_extension_fixture.py"), include_str!("fixtures/extension.py"))
            .unwrap();
        fs::write(
            metadata.join("METADATA"),
            "Metadata-Version: 2.1\nName: hy-extension-fixture\nVersion: 1.2.3\n",
        )
        .unwrap();
        fs::write(
            metadata.join("entry_points.txt"),
            "[hcli.extensions]\nfixture = hy_extension_fixture:register\n",
        )
        .unwrap();
        Self {
            sandbox,
            python,
            site,
        }
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = self.sandbox.command(args);
        command
            .env("HCLI_EXTENSION_PYTHON", &self.python)
            .env("PYTHONPATH", &self.site)
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .env_remove("HY_EXTENSION_BROKEN")
            .env_remove("HY_EXTENSION_REPLACE")
            .env_remove("HY_EXTENSION_REMOVE");
        command.env_remove("HY_EXTENSION_EXIT");
        command.env_remove("HY_EXTENSION_TRACE");
        command
    }
}

#[test]
#[ignore = "requires HY_TEST_EXTENSION_PYTHON with pinned ida-hcli installed"]
fn extensions_are_discovered_in_help_list_and_sorted_command_inventory() {
    let fixture = RuntimeFixture::new();
    let output = fixture.command(&["extension", "list"]).output().unwrap();
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("Extensions: fixture"));
    let output = fixture.command(&["--help"]).output().unwrap();
    assert_success(&output);
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("fixture [v1.2.3]"), "{text}");
    assert!(text.contains("laboratory"), "{text}");
    let output = fixture.command(&["commands"]).output().unwrap();
    assert_success(&output);
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("hy laboratory echo"), "{text}");
    assert!(text.contains("hy ida extension-fixture"), "{text}");
    assert!(!text.contains("laboratory hidden"), "{text}");
    let lines: Vec<_> = text.lines().filter(|line| line.starts_with("hy ")).collect();
    assert!(lines.windows(2).all(|pair| pair[0] < pair[1]), "{text}");
}

#[test]
#[ignore = "requires HY_TEST_EXTENSION_PYTHON with pinned ida-hcli installed"]
fn extension_arguments_context_and_exit_status_are_preserved_without_repeated_callbacks() {
    let fixture = RuntimeFixture::new();
    let output = fixture
        .command(&[
            "--auth",
            "key",
            "--auth-credentials",
            "selected",
            "laboratory",
            "echo",
            "--label",
            "two words",
            "--label=second",
            "--exit-code",
            "7",
            "--",
            "-literal",
            "space path",
            "λ",
        ])
        .env("HCLI_BINARY_NAME", "hx")
        .env("HY_EXTENSION_TRACE", "1")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(7), "{}", String::from_utf8_lossy(&output.stderr));
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["labels"], serde_json::json!(["two words", "second"]));
    assert_eq!(value["values"], serde_json::json!(["-literal", "space path", "λ"]));
    assert_eq!(value["auth"], "key");
    assert_eq!(value["credentials"], "selected");
    assert_eq!(value["program"], "hx");
    assert_eq!(value["registrations"], 1);
    assert_eq!(value["parameter_callbacks"], 1);
    let trace = String::from_utf8_lossy(&output.stderr);
    let registrations: Vec<_> =
        trace.lines().filter(|line| line.starts_with("registered: ")).collect();
    assert_eq!(registrations, vec![format!("registered: {}", value["execution_pid"]).as_str()]);
}

#[test]
#[ignore = "requires HY_TEST_EXTENSION_PYTHON with pinned ida-hcli installed"]
fn nested_commands_overrides_and_removals_follow_the_registered_tree() {
    let fixture = RuntimeFixture::new();
    let output = fixture
        .command(&["ida", "extension-fixture", "--message", "nested output"])
        .output()
        .unwrap();
    assert_success(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "nested output");
    let output = fixture.command(&["whoami"]).env("HY_EXTENSION_REPLACE", "1").output().unwrap();
    assert_success(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "extension identity");
    let output =
        fixture.command(&["license", "list"]).env("HY_EXTENSION_REMOVE", "1").output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("No such command"));
    let output = fixture.command(&["commands"]).env("HY_EXTENSION_REMOVE", "1").output().unwrap();
    assert_success(&output);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("hy license "));
}

#[test]
#[ignore = "requires HY_TEST_EXTENSION_PYTHON with pinned ida-hcli installed"]
fn extension_prompts_keep_the_controlling_terminal() {
    let fixture = RuntimeFixture::new();
    let mut terminal = Terminal::start(fixture.command(&["laboratory", "prompt"]));
    terminal.wait_for("Fixture value");
    terminal.send("two words\n");
    let (status, output) = terminal.finish();
    assert!(status.success(), "{output}");
    assert!(output.contains("Received: two words"), "{output}");
}

#[test]
#[ignore = "requires HY_TEST_EXTENSION_PYTHON with pinned ida-hcli installed"]
fn extension_help_uses_its_real_options_and_unmodified_commands_use_rust() {
    let fixture = RuntimeFixture::new();
    let output = fixture.command(&["laboratory", "echo", "--help"]).output().unwrap();
    assert_success(&output);
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("--label") && text.contains("--exit-code"), "{text}");
    let output = fixture.command(&["--version"]).output().unwrap();
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("hy "));
    let output = fixture.command(&["ke", "ida", "list"]).output().unwrap();
    assert_success(&output);
    // The current Python host has no `ke` group; successful dispatch proves Rust handled it.
    assert!(String::from_utf8_lossy(&output.stderr).contains("No IDA instances registered"));
}

#[test]
#[ignore = "requires HY_TEST_EXTENSION_PYTHON with pinned ida-hcli installed"]
fn failed_registration_is_an_error_instead_of_an_empty_catalog() {
    let fixture = RuntimeFixture::new();
    let output =
        fixture.command(&["extension", "list"]).env("HY_EXTENSION_BROKEN", "1").output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("fixture registration failed"));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("No extensions installed"));
    for code in ["0", "7"] {
        let output = fixture
            .command(&["extension", "list"])
            .env("HY_EXTENSION_EXIT", code)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(code.parse().unwrap()));
        assert!(!String::from_utf8_lossy(&output.stdout).contains("No extensions installed"));
    }
    // An entry point without a usable HCLI host is an error, not an empty inventory.
    let shadow = fixture.site.join("hcli/lib");
    fs::create_dir_all(&shadow).unwrap();
    fs::write(fixture.site.join("hcli/__init__.py"), "").unwrap();
    fs::write(shadow.join("__init__.py"), "").unwrap();
    let output = fixture.command(&["extension", "list"]).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("hcli.lib.extensions"));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("No extensions installed"));
}

#[test]
fn disabled_extension_runtime_preserves_native_help_without_python() {
    let sandbox = Sandbox::new();
    let output = sandbox.command(&["--help"]).env("PATH", sandbox.path()).output().unwrap();
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("Usage:"));
    assert!(!sandbox.config_path().exists());
}

#[test]
fn explicitly_missing_extension_runtime_fails_with_its_path() {
    let sandbox = Sandbox::new();
    let missing = sandbox.path().join("missing-python");
    let output = sandbox
        .command(&["extension", "list"])
        .env("HCLI_EXTENSION_PYTHON", &missing)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains(missing.to_str().unwrap()));
    assert!(!sandbox.config_path().exists());
}

#[test]
#[ignore = "requires HY_TEST_EXTENSION_PYTHON with pinned ida-hcli installed"]
fn path_runtime_discovery_and_empty_catalog_do_not_require_explicit_configuration() {
    let fixture = RuntimeFixture::new();
    let output = fixture
        .command(&["extension", "list"])
        .env_remove("HCLI_EXTENSION_PYTHON")
        .env("PATH", fixture.python.parent().unwrap())
        .output()
        .unwrap();
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("Extensions: fixture"));

    fs::create_dir_all(fixture.sandbox.config_path().parent().unwrap()).unwrap();
    fs::write(fixture.sandbox.config_path(), b"malformed configuration").unwrap();
    let output = fixture.command(&["--help"]).env_remove("PYTHONPATH").output().unwrap();
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("Usage:"));
    assert_eq!(fs::read(fixture.sandbox.config_path()).unwrap(), b"malformed configuration");
}
