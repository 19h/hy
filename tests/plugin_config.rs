//! Configuration defaults, validation, and publication failure boundaries.

mod support;

use std::fs;

use serde_json::json;
use support::*;

#[test]
fn defaults_follow_the_installed_descriptor_until_explicitly_overridden() {
    let sandbox = Sandbox::new();
    let package = sandbox.path().join("package.zip");
    let mut manifest = identity_manifest("1.0", "https://github.com/example/plugin");
    manifest["plugin"]["settings"] = json!([
        {"key": "enabled", "name": "Enabled", "type": "boolean", "required": false, "default": false},
    ]);
    archive_manifest(&package, &manifest, &[]);
    assert_success(&sandbox.run(&["plugin", "install", package.to_str().unwrap()]));
    let exported = sandbox.run(&["plugin", "config", "EXAMPLE", "export"]);
    assert_success(&exported);
    assert_eq!(serde_json::from_slice::<serde_json::Value>(&exported.stdout).unwrap(), json!({}));

    manifest["plugin"]["version"] = json!("2.0");
    manifest["plugin"]["settings"][0]["default"] = json!(true);
    archive_manifest(&package, &manifest, &[]);
    assert_success(&sandbox.run(&["plugin", "install", "-U", package.to_str().unwrap()]));
    let value = sandbox.run(&["plugin", "config", "EXAMPLE", "get", "enabled"]);
    assert_success(&value);
    assert_eq!(String::from_utf8_lossy(&value.stdout).trim(), "true");

    assert_success(&sandbox.run(&["plugin", "config", "EXAMPLE", "set", "enabled", "FALSE"]));
    let config: serde_json::Value =
        serde_json::from_slice(&fs::read(sandbox.path().join("idausr/ida-config.json")).unwrap())
            .unwrap();
    assert_eq!(config["Plugins"]["example"]["settings"]["enabled"], false);
    assert!(config["Plugins"].get("EXAMPLE").is_none());
    assert_success(&sandbox.run(&["plugin", "config", "example", "del", "enabled"]));
    let value = sandbox.run(&["plugin", "config", "example", "get", "enabled"]);
    assert_success(&value);
    assert_eq!(String::from_utf8_lossy(&value.stdout).trim(), "true");
}

#[test]
fn invalid_configuration_import_is_atomic() {
    let sandbox = Sandbox::new();
    let package = sandbox.path().join("package.zip");
    archive(&package, "1.0", &[]);
    assert_success(&sandbox.run(&["plugin", "install", package.to_str().unwrap()]));
    assert_success(&sandbox.run(&["plugin", "config", "example", "set", "enabled", "true"]));
    let path = sandbox.path().join("idausr/ida-config.json");
    let original = fs::read(&path).unwrap();
    for payload in [r#"{"enabled":false,"unknown":"invalid"}"#, r#"{"enabled":"false"}"#, "[]"] {
        let output = sandbox.run(&["plugin", "config", "example", "import", payload]);
        assert!(!output.status.success(), "accepted {payload}");
        assert_eq!(fs::read(&path).unwrap(), original);
    }
    for args in [
        vec!["plugin", "config", "example", "get", "unknown"],
        vec!["plugin", "config", "example", "set", "enabled", "yes"],
        vec!["plugin", "config", "missing", "set", "enabled", "true"],
    ] {
        assert!(!sandbox.run(&args).status.success());
        assert_eq!(fs::read(&path).unwrap(), original);
    }
}

#[test]
fn listing_preserves_descriptor_order_and_redacts_secrets() {
    let sandbox = Sandbox::new();
    let package = sandbox.path().join("package.zip");
    let mut manifest = identity_manifest("1.0", "https://github.com/example/plugin");
    manifest["plugin"]["settings"] = json!([
        {"key": "z-secret", "name": "Secret", "type": "string", "required": false, "secret": true, "default": "fixture-secret"},
        {"key": "a-unset", "name": "Unset", "type": "string", "required": false},
    ]);
    archive_manifest(&package, &manifest, &[]);
    assert_success(&sandbox.run(&["plugin", "install", package.to_str().unwrap()]));
    let output = sandbox.run(&["plugin", "config", "example", "list"]);
    assert_success(&output);
    let output = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output.contains("fixture-secret"));
    assert!(output.contains("********"));
    assert!(output.contains("<not set>"));
    assert!(output.find("z-secret").unwrap() < output.find("a-unset").unwrap());
    let setup = sandbox.run(&["plugin", "config", "example", "setup"]);
    assert!(!setup.status.success());
    assert!(String::from_utf8_lossy(&setup.stderr).contains("interactive terminal"));
}

#[test]
fn configuration_failure_removes_a_fresh_install_but_retains_a_completed_upgrade() {
    for upgrade in [false, true] {
        let sandbox = Sandbox::new();
        let package = sandbox.path().join("package.zip");
        let mut manifest = identity_manifest("1.0", "https://github.com/example/plugin");
        if upgrade {
            archive_manifest(&package, &manifest, &[]);
            assert_success(&sandbox.run(&["plugin", "install", package.to_str().unwrap()]));
        }
        manifest["plugin"]["version"] = json!("2.0");
        manifest["plugin"]["settings"] = json!([
            {"key": "token", "name": "Token", "type": "string", "required": true},
        ]);
        archive_manifest(&package, &manifest, &[]);
        let output = sandbox.run(&["plugin", "install", "-U", package.to_str().unwrap()]);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("required settings missing"));
        if upgrade {
            assert_eq!(installed_manifest(&sandbox)["plugin"]["version"], "2.0");
        } else {
            assert!(!sandbox.path().join("idausr/plugins/example").exists());
        }
    }
}

#[test]
fn explicit_install_configuration_does_not_prompt_for_other_required_settings() {
    let sandbox = Sandbox::new();
    let package = sandbox.path().join("package.zip");
    let mut manifest = identity_manifest("1.0", "https://github.com/example/plugin");
    manifest["plugin"]["settings"] = json!([
        {"key": "enabled", "name": "Enabled", "type": "boolean", "required": false, "default": false},
        {"key": "token", "name": "Token", "type": "string", "required": true},
    ]);
    archive_manifest(&package, &manifest, &[]);
    assert_success(&sandbox.run(&[
        "plugin",
        "install",
        "--config",
        "enabled=false",
        package.to_str().unwrap(),
    ]));
    assert!(!sandbox.path().join("idausr/ida-config.json").exists());
}

#[test]
fn repeated_install_options_validate_only_the_final_string_value() {
    let sandbox = Sandbox::new();
    let package = sandbox.path().join("package.zip");
    let mut manifest = identity_manifest("1.0", "https://github.com/example/plugin");
    manifest["plugin"]["settings"] = json!([
        {"key":"token","name":"Token","type":"string","required":true,"validation_pattern":r"(?P<part>[a-z]+)-(?P=part)\Z"},
    ]);
    archive_manifest(&package, &manifest, &[]);
    assert_success(&sandbox.run(&[
        "plugin",
        "install",
        "--config",
        "token=invalid-value",
        "--config",
        "token=same-same",
        package.to_str().unwrap(),
    ]));
    let value = sandbox.run(&["plugin", "config", "example", "get", "token"]);
    assert_success(&value);
    assert_eq!(String::from_utf8_lossy(&value.stdout).trim(), "same-same");
    let output = sandbox.run(&["plugin", "config", "example", "set", "token", "different-value"]);
    assert!(!output.status.success());
}

#[test]
fn malformed_boolean_install_option_fails_even_if_a_later_option_would_replace_it() {
    let sandbox = Sandbox::new();
    let package = sandbox.path().join("package.zip");
    archive(&package, "1.0", &[]);
    let output = sandbox.run(&[
        "plugin",
        "install",
        "--config",
        "enabled=yes",
        "--config",
        "enabled=false",
        package.to_str().unwrap(),
    ]);
    assert!(!output.status.success());
    assert!(!sandbox.path().join("idausr/plugins/example").exists());
}

#[test]
fn empty_import_argument_reads_settings_from_standard_input() {
    use std::io::Write;
    use std::process::Stdio;
    let sandbox = Sandbox::new();
    let package = sandbox.path().join("package.zip");
    archive(&package, "1.0", &[]);
    assert_success(&sandbox.run(&["plugin", "install", package.to_str().unwrap()]));
    let mut child = sandbox
        .command(&["plugin", "config", "example", "import", ""])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(br#"{"enabled":true}"#).unwrap();
    assert_success(&child.wait_with_output().unwrap());
    let value = sandbox.run(&["plugin", "config", "example", "get", "enabled"]);
    assert_success(&value);
    assert_eq!(String::from_utf8_lossy(&value.stdout).trim(), "true");
}
