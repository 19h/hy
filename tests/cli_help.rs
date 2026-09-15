//! Help and command inventory use local state without authentication or persistence.

mod support;

use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use support::{
    auth::*,
    http::{Response, Server},
    *,
};

fn installation(sandbox: &Sandbox, name: &str, sdk: Option<&str>) -> PathBuf {
    let directory = sandbox.path().join(name);
    let binaries = if cfg!(target_os = "macos") {
        directory.join("Contents/MacOS")
    } else {
        directory.clone()
    };
    fs::create_dir_all(binaries.join("python")).unwrap();
    let binary = if cfg!(windows) {
        "ida.exe"
    } else {
        "ida"
    };
    fs::write(binaries.join(binary), b"inert fixture 9.2.260101").unwrap();
    if let Some(sdk) = sdk {
        fs::write(binaries.join("python/ida_pro.py"), format!("\"\"\"\nIDA SDK v{sdk}.\n\"\"\"\n"))
            .unwrap();
    }
    directory
}

fn ida_config(sandbox: &Sandbox, path: &Path) {
    let directory = sandbox.path().join("idausr");
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        directory.join("ida-config.json"),
        serde_json::to_vec(&json!({
            "Paths": {"ida-install-dir": path}
        }))
        .unwrap(),
    )
    .unwrap();
}

#[test]
fn root_help_shows_local_authentication_and_missing_ida_without_requests_or_writes() {
    for authenticated in [false, true] {
        let sandbox = Sandbox::new();
        if authenticated {
            write_config(&sandbox, &stored("interactive", "not-a-validated-token"));
        }
        let before = fs::read(sandbox.config_path()).ok();
        let server = Server::start(|_, _| Response::missing());
        let output = command(&sandbox, &server, &["--help"]).output().unwrap();
        assert_success(&output);
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(text.contains("Status:"), "{text}");
        assert!(
            text.contains(if authenticated {
                EMAIL
            } else {
                "Not logged in"
            }),
            "{text}"
        );
        assert!(text.contains("Not installed"), "{text}");
        assert!(!text.contains("not-a-validated-token"));
        assert!(server.requests().is_empty());
        assert_eq!(fs::read(sandbox.config_path()).ok(), before);
        assert!(!sandbox.path().join("idausr").exists());
    }
}

#[test]
fn root_help_reports_default_ida_sdk_version_and_independent_idalib_state() {
    let sandbox = Sandbox::new();
    let directory = installation(&sandbox, "IDA 9.1.app", Some("9.3"));
    let library = if cfg!(target_os = "macos") {
        "Contents/MacOS/libidalib.dylib"
    } else if cfg!(windows) {
        "idalib.dll"
    } else {
        "libidalib.so"
    };
    fs::write(directory.join(library), b"inert library").unwrap();
    write_config(
        &sandbox,
        &json!({"ida.instances": {"default-9.0": directory}, "ida.default": "default-9.0"}),
    );
    ida_config(&sandbox, &directory);
    let before = fs::read(sandbox.config_path()).unwrap();
    let output =
        sandbox.command(&["--help"]).env("HCLI_API_KEY", "private-fixture-key").output().unwrap();
    assert_success(&output);
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("API key configured"), "{text}");
    assert!(!text.contains("private-fixture-key"));
    assert!(text.contains("IDA 9.3 at"), "{text}");
    assert!(text.contains("idalib: active"), "{text}");
    assert!(!text.contains("idalib: active ("), "{text}");
    assert_eq!(fs::read(sandbox.config_path()).unwrap(), before);

    ida_config(&sandbox, &sandbox.path().join("missing"));
    let output = sandbox.run(&["--help"]);
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("idalib: not found"));
}

#[test]
fn root_help_distinguishes_missing_defaults_from_unselected_installations() {
    let sandbox = Sandbox::new();
    let directory = installation(&sandbox, "fixture.app", None);
    let mut config =
        json!({"ida.instances": {"valid": directory, "broken": sandbox.path().join("absent")}});
    write_config(&sandbox, &config);
    let output = sandbox.run(&["--help"]);
    assert_success(&output);
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("2 instance(s), no default set (1 valid)"), "{text}");
    assert!(text.contains("idalib: not configured"), "{text}");
    config["ida.default"] = json!("broken");
    write_config(&sandbox, &config);
    let output = sandbox.run(&["--help"]);
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("IDA: Not found"));
}

#[test]
fn malformed_configuration_does_not_hide_help_or_get_rewritten() {
    let sandbox = Sandbox::new();
    fs::create_dir_all(sandbox.config_path().parent().unwrap()).unwrap();
    fs::write(sandbox.config_path(), b"malformed configuration").unwrap();
    let output = sandbox.run(&["--help"]);
    assert_success(&output);
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("Usage:"));
    assert!(!text.contains("Status:"));
    assert_eq!(fs::read(sandbox.config_path()).unwrap(), b"malformed configuration");
}

#[test]
fn version_overrides_and_sorted_command_inventory_use_configured_identity() {
    let sandbox = Sandbox::new();
    let output = sandbox
        .command(&["--version"])
        .env("HCLI_VERSION", "9.8.7")
        .env("HCLI_VERSION_EXTRA", "+fixture")
        .env("HCLI_BINARY_NAME", "hx")
        .output()
        .unwrap();
    assert_success(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hx 9.8.7+fixture");
    assert!(!sandbox.config_path().exists());
    let output = sandbox.command(&["commands"]).env("HCLI_BINARY_NAME", "hx").output().unwrap();
    assert_success(&output);
    let text = String::from_utf8_lossy(&output.stdout);
    let rows: Vec<_> = text.lines().filter(|line| line.starts_with("hx ")).collect();
    assert!(!rows.is_empty());
    let mut sorted = rows.clone();
    sorted.sort();
    assert_eq!(rows, sorted);
    assert!(!text.contains("hx asset "));
    assert!(text.contains("hx license get"));
    assert!(text.contains(&format!("Total commands: {}", rows.len())));
}

#[test]
fn unsupported_global_quiet_flag_is_rejected_instead_of_silently_ignored() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&["--quiet", "commands"]);
    assert!(!output.status.success());
    assert!(!sandbox.config_path().exists());
}
