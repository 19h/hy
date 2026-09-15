//! Instance registration, ordering and idalib activation through isolated CLI processes.
#![cfg(unix)]

mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use serde_json::{Value, json};
use support::terminal::Terminal;
use support::*;

fn installation(sandbox: &Sandbox, name: &str, version: &str) -> PathBuf {
    let path = sandbox.path().join(name);
    let executable_dir = executable_directory(&path);
    fs::create_dir_all(executable_dir.join("python")).unwrap();
    fs::write(executable_dir.join("ida"), b"fixture binary").unwrap();
    fs::write(executable_dir.join("python/ida_pro.py"), format!("# IDA SDK v{version}\n")).unwrap();
    path.canonicalize().unwrap()
}

fn executable_directory(path: &Path) -> PathBuf {
    if cfg!(target_os = "macos") && path.extension().is_some_and(|suffix| suffix == "app") {
        path.join("Contents/MacOS")
    } else {
        path.to_owned()
    }
}

fn save_config(sandbox: &Sandbox, instances: Value, default: Option<&str>) {
    let mut config = json!({"ida.instances": instances, "sentinel": "retain"});
    if let Some(default) = default {
        config["ida.default"] = json!(default);
    }
    fs::create_dir_all(sandbox.config_path().parent().unwrap()).unwrap();
    fs::write(sandbox.config_path(), serde_json::to_vec(&config).unwrap()).unwrap();
}

fn config(sandbox: &Sandbox) -> Value {
    serde_json::from_slice(&fs::read(sandbox.config_path()).unwrap()).unwrap()
}

fn idalib_config_path(sandbox: &Sandbox) -> PathBuf {
    sandbox.path().join("idausr/ida-config.json")
}

fn save_idalib_config(sandbox: &Sandbox) -> Vec<u8> {
    let path = idalib_config_path(sandbox);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let bytes = br#"{"Paths":{"ida-install-dir":"/previous/idalib","other":"retain"}}"#;
    fs::write(path, bytes).unwrap();
    bytes.to_vec()
}

fn output_text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn manual_registration_resolves_home_and_symlinks_without_selecting_a_default() {
    let sandbox = Sandbox::new();
    let path = installation(&sandbox, "custom", "9.4");
    std::os::unix::fs::symlink(&path, sandbox.path().join("link")).unwrap();
    assert_success(&sandbox.run(&["ida", "add", "custom", "~/link"]));
    let saved = config(&sandbox);
    assert_eq!(saved["ida.instances"], json!({"custom": path}));
    assert!(saved.get("ida.default").is_none());
    assert!(!idalib_config_path(&sandbox).exists());
}

#[test]
fn duplicate_names_and_invalid_registration_paths_preserve_configuration_bytes() {
    let sandbox = Sandbox::new();
    let replacement = installation(&sandbox, "replacement", "9.4");
    save_config(&sandbox, json!({"existing": "/original/path"}), Some("existing"));
    let before = fs::read(sandbox.config_path()).unwrap();
    let duplicate = sandbox.run(&["ke", "ida", "add", "existing", replacement.to_str().unwrap()]);
    assert_success(&duplicate);
    assert!(output_text(&duplicate).contains("already exists"));
    assert_eq!(fs::read(sandbox.config_path()).unwrap(), before);

    let invalid_directory = sandbox.path().join("empty-directory");
    fs::create_dir(&invalid_directory).unwrap();
    let plain_file = sandbox.path().join("file");
    fs::write(&plain_file, b"not a directory").unwrap();
    for path in [invalid_directory, plain_file, sandbox.path().join("missing")] {
        let output = sandbox.run(&["ida", "add", "new", path.to_str().unwrap()]);
        assert!(!output.status.success(), "{}", output_text(&output));
        assert_eq!(fs::read(sandbox.config_path()).unwrap(), before);
    }
}

#[test]
fn invalid_instance_arguments_fail_before_changing_configuration() {
    let sandbox = Sandbox::new();
    save_config(&sandbox, json!({"registered": "/fixture/path"}), Some("registered"));
    let before = fs::read(sandbox.config_path()).unwrap();
    for args in [
        vec!["ida", "add"],
        vec!["ida", "add", "name"],
        vec!["ida", "remove"],
        vec!["ida", "remove", "registered", "--all"],
        vec!["ida", "remove", "absent"],
        vec!["ida", "switch", "absent"],
    ] {
        let output = sandbox.run(&args);
        assert!(!output.status.success(), "{args:?}: {}", output_text(&output));
        assert_eq!(fs::read(sandbox.config_path()).unwrap(), before);
    }
}

#[test]
fn removing_the_default_uses_numeric_versions_and_descending_names_including_stale_paths() {
    let sandbox = Sandbox::new();
    save_config(
        &sandbox,
        json!({
            "selected": "/missing/current",
            "a-9.10": "/missing/a",
            "z-9.10": "/missing/z",
            "last-9.9": "/missing/older",
            "a-0.0": "/missing/zero",
            "zz-unknown": "/missing/unknown",
        }),
        Some("selected"),
    );
    let idalib_before = save_idalib_config(&sandbox);
    for (removed, expected) in [
        ("selected", Some("z-9.10")),
        ("z-9.10", Some("a-9.10")),
        ("a-9.10", Some("last-9.9")),
        ("last-9.9", Some("a-0.0")),
        ("a-0.0", Some("zz-unknown")),
        ("zz-unknown", None),
    ] {
        assert_success(&sandbox.run(&["ida", "remove", removed]));
        let saved = config(&sandbox);
        assert_eq!(saved["ida.default"].as_str(), expected);
        assert!(saved["ida.instances"].get(removed).is_none());
        assert_eq!(saved["sentinel"], "retain");
    }
    assert_eq!(fs::read(idalib_config_path(&sandbox)).unwrap(), idalib_before);
    assert_success(&sandbox.run(&["ida", "remove", "absent"]));
    assert!(!sandbox.run(&["ida", "switch"]).status.success());
}

#[test]
fn removal_retains_installation_files_and_a_separate_default() {
    let sandbox = Sandbox::new();
    let path = installation(&sandbox, "retained", "9.4");
    save_config(&sandbox, json!({"remove": path, "default": "/missing"}), Some("default"));
    assert_success(&sandbox.run(&["ida", "remove", "remove"]));
    assert_eq!(config(&sandbox)["ida.default"], "default");
    assert!(path.join("ida").is_file());
    assert_success(&sandbox.run(&["ida", "remove", "--all"]));
    assert_eq!(config(&sandbox)["ida.instances"], json!({}));
    assert!(config(&sandbox).get("ida.default").is_none());
    assert!(path.join("ida").is_file());
}

#[test]
fn inventory_sorts_by_sdk_version_then_name_and_reports_invalid_and_missing_paths() {
    let sandbox = Sandbox::new();
    let older = installation(&sandbox, "older", "9.9");
    let newer = installation(&sandbox, "newer", "9.10");
    let invalid = sandbox.path().join("invalid");
    fs::create_dir(&invalid).unwrap();
    save_config(
        &sandbox,
        json!({"z-newer": newer, "older": older, "a-newer": newer, "missing": "/missing", "invalid": invalid}),
        Some("older"),
    );
    let output = sandbox.run(&["ida", "list"]);
    assert_success(&output);
    let text = output_text(&output);
    let names = ["a-newer", "z-newer", "older (default)", "invalid", "missing"];
    let positions: Vec<_> = names.iter().map(|name| text.find(name).unwrap()).collect();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]), "{text}");
    assert!(text.contains("3/5 instances are valid"), "{text}");
    assert!(text.contains("Invalid") && text.contains("Missing"), "{text}");
    assert!(text.contains("ida switch a-newer"), "{text}");
}

#[test]
fn switching_activates_only_the_current_platforms_idalib_and_preserves_other_fields() {
    let sandbox = Sandbox::new();
    let path = installation(&sandbox, "selected", "9.4");
    save_config(&sandbox, json!({"selected": path}), None);
    let before = save_idalib_config(&sandbox);
    fs::write(path.join("idalib.dll"), b"foreign platform library").unwrap();
    assert_success(&sandbox.run(&["ida", "switch", "selected"]));
    assert_eq!(config(&sandbox)["ida.default"], "selected");
    assert_eq!(fs::read(idalib_config_path(&sandbox)).unwrap(), before);
    let library = if cfg!(target_os = "macos") {
        "libidalib.dylib"
    } else {
        "libidalib.so"
    };
    fs::write(path.join(library), b"native library fixture").unwrap();
    assert_success(&sandbox.run(&["ida", "switch", "selected"]));
    let saved: Value =
        serde_json::from_slice(&fs::read(idalib_config_path(&sandbox)).unwrap()).unwrap();
    assert_eq!(saved["Paths"]["ida-install-dir"], json!(path));
    assert_eq!(saved["Paths"]["other"], "retain");
}

#[test]
fn interactive_switch_preselects_the_current_default_and_cancellation_preserves_bytes() {
    let sandbox = Sandbox::new();
    save_config(
        &sandbox,
        json!({"first": "/missing/first", "selected": "/missing/selected"}),
        Some("selected"),
    );
    let mut terminal = Terminal::start(sandbox.command(&["ida", "switch"]));
    terminal.wait_for("CURRENT DEFAULT");
    terminal.send("\n");
    let (status, text) = terminal.finish();
    assert!(status.success(), "{text}");
    assert_eq!(config(&sandbox)["ida.default"], "selected");
    let before = fs::read(sandbox.config_path()).unwrap();
    let mut terminal = Terminal::start(sandbox.command(&["ida", "switch"]));
    terminal.wait_for("CURRENT DEFAULT");
    terminal.send("q");
    let (status, text) = terminal.finish();
    assert!(status.success(), "{text}");
    assert_eq!(fs::read(sandbox.config_path()).unwrap(), before);
}

#[test]
fn deprecated_set_default_is_independent_of_instance_selection_and_rejects_invalid_paths() {
    let sandbox = Sandbox::new();
    let path = installation(&sandbox, "selected", "9.4");
    save_config(&sandbox, json!({"selected": path}), Some("selected"));
    let registry_before = fs::read(sandbox.config_path()).unwrap();
    let before = save_idalib_config(&sandbox);
    let output = sandbox.run(&["ida", "set-default"]);
    assert_success(&output);
    assert!(output_text(&output).contains("Default IDA installation: /previous/idalib"));
    for invalid in [sandbox.path().join("missing"), sandbox.path().to_owned()] {
        assert_success(&sandbox.run(&["ida", "set-default", invalid.to_str().unwrap()]));
        assert_eq!(fs::read(idalib_config_path(&sandbox)).unwrap(), before);
    }
    assert_success(&sandbox.run(&["ida", "set-default", "~/selected"]));
    let saved: Value =
        serde_json::from_slice(&fs::read(idalib_config_path(&sandbox)).unwrap()).unwrap();
    assert_eq!(saved["Paths"]["ida-install-dir"], json!(path));
    assert_eq!(fs::read(sandbox.config_path()).unwrap(), registry_before);
}

#[test]
fn cancelling_discovery_does_not_register_the_preselected_installations() {
    let sandbox = Sandbox::new();
    let path = installation(&sandbox, "custom", "9.4");
    save_config(&sandbox, json!({"sentinel": "/missing"}), None);
    let before = fs::read(sandbox.config_path()).unwrap();
    let mut command = sandbox.command(&["ida", "add", "--auto"]);
    command.env("HCLI_CURRENT_IDA_INSTALL_DIR", path);
    let mut terminal = Terminal::start(command);
    terminal.wait_for("Select installations to register");
    terminal.send("q");
    let (status, text) = terminal.finish();
    assert!(status.success(), "{text}");
    assert_eq!(fs::read(sandbox.config_path()).unwrap(), before);
}

#[test]
fn empty_discovery_selection_remains_pending_and_can_be_cancelled_without_writes() {
    let sandbox = Sandbox::new();
    let path = installation(&sandbox, "custom", "9.4");
    save_config(&sandbox, json!({"sentinel": "/missing"}), None);
    let before = fs::read(sandbox.config_path()).unwrap();
    let mut command = sandbox.command(&["ida", "add", "--auto"]);
    command.env("HCLI_CURRENT_IDA_INSTALL_DIR", path);
    let mut terminal = Terminal::start(command);
    terminal.wait_for("Select installations to register");
    terminal.send("a\n");
    terminal.wait_for("Please select at least one installation");
    assert_eq!(fs::read(sandbox.config_path()).unwrap(), before);
    terminal.send("q");
    let (status, text) = terminal.finish();
    assert!(status.success(), "{text}");
    assert_eq!(fs::read(sandbox.config_path()).unwrap(), before);
}

#[test]
fn failed_idalib_configuration_preserves_its_bytes_and_reports_partial_switch_failure() {
    let sandbox = Sandbox::new();
    let path = installation(&sandbox, "selected", "9.4");
    let library = if cfg!(target_os = "macos") {
        "libidalib.dylib"
    } else {
        "libidalib.so"
    };
    fs::write(path.join(library), b"fixture library").unwrap();
    save_config(&sandbox, json!({"selected": path}), None);
    let config_path = idalib_config_path(&sandbox);
    fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    fs::write(&config_path, b"{malformed").unwrap();
    let output = sandbox.run(&["ida", "switch", "selected"]);
    assert!(!output.status.success());
    assert!(!output_text(&output).contains("Default IDA instance set"));
    assert_eq!(fs::read(config_path).unwrap(), b"{malformed");
    // Upstream commits ida.default before attempting the independent idalib file.
    assert_eq!(config(&sandbox)["ida.default"], "selected");
}

#[cfg(target_os = "macos")]
#[test]
fn automatic_registration_selects_the_highest_new_version_and_skips_duplicate_names() {
    use std::os::unix::fs::PermissionsExt;

    let sandbox = Sandbox::new();
    let older = installation(&sandbox, "IDA Professional 9.9.app", "9.9");
    let newer = installation(&sandbox, "IDA Professional 9.10.app", "9.10");
    let bin = sandbox.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let mdfind = bin.join("mdfind");
    fs::write(&mdfind, "#!/bin/sh\nprintf '%s\\n' \"$HY_FIXTURE_SPOTLIGHT_PATH\"\n").unwrap();
    fs::set_permissions(&mdfind, fs::Permissions::from_mode(0o755)).unwrap();
    save_config(&sandbox, json!({"existing-99.0": "/missing"}), None);
    let mut command = sandbox.command(&["ida", "add", "--auto"]);
    command
        .env("PATH", &bin)
        .env("HCLI_CURRENT_IDA_INSTALL_DIR", &older)
        .env("HY_FIXTURE_SPOTLIGHT_PATH", &newer);
    let mut terminal = Terminal::start(command);
    terminal.wait_for("Select installations to register");
    // Every discovered name is initially new: clear all and select the first two only.
    terminal.send("a j \n");
    let (status, text) = terminal.finish();
    assert!(status.success(), "{text}");
    assert_eq!(config(&sandbox)["ida.default"], "ida-pro-9.10");
    assert_eq!(config(&sandbox)["ida.instances"].as_object().unwrap().len(), 3);

    save_config(&sandbox, json!({"ida-pro-9.9": "/original/path"}), None);
    let mut command = sandbox.command(&["ida", "add", "--auto"]);
    command
        .env("PATH", &bin)
        .env("HCLI_CURRENT_IDA_INSTALL_DIR", &older)
        .env("HY_FIXTURE_SPOTLIGHT_PATH", &newer);
    let mut terminal = Terminal::start(command);
    terminal.wait_for("already registered");
    // Mixed defaults: select all, clear all, then explicitly select the first two.
    terminal.send("aa j \n");
    let (status, text) = terminal.finish();
    assert!(status.success(), "{text}");
    let saved = config(&sandbox);
    assert_eq!(saved["ida.instances"]["ida-pro-9.9"], "/original/path");
    assert_eq!(saved["ida.instances"]["ida-pro-9.10"], json!(newer));
    assert_eq!(saved["ida.default"], "ida-pro-9.10");
}
