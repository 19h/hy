//! Actual terminal configuration, cancellation and install rollback contracts.
#![cfg(unix)]

mod support;

use serde_json::{Value, json};
use std::fs;
use support::terminal::Terminal;
use support::*;

fn installed(sandbox: &Sandbox, settings: Value) {
    let mut manifest = identity_manifest("1.0", "https://github.com/example/plugin");
    manifest["plugin"]["settings"] = settings;
    let directory = sandbox.path().join("idausr/plugins/example");
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("ida-plugin.json"), serde_json::to_vec(&manifest).unwrap()).unwrap();
    fs::write(directory.join("plugin.py"), b"# fixture\n").unwrap();
}

fn settings_path(sandbox: &Sandbox) -> std::path::PathBuf {
    sandbox.path().join("idausr/ida-config.json")
}

fn saved_settings(sandbox: &Sandbox, values: Value) -> Vec<u8> {
    let bytes = serde_json::to_vec(
        &json!({"Plugins": {"example": {"settings": values}}, "Paths": {"sentinel": "retain"}}),
    )
    .unwrap();
    fs::write(settings_path(sandbox), &bytes).unwrap();
    bytes
}

fn read_settings(sandbox: &Sandbox) -> Value {
    let config: Value = serde_json::from_slice(&fs::read(settings_path(sandbox)).unwrap()).unwrap();
    config["Plugins"]["example"]["settings"].clone()
}

#[test]
fn setup_can_clear_prefilled_text_and_preserves_existing_choices_booleans_and_secrets() {
    let sandbox = Sandbox::new();
    installed(
        &sandbox,
        json!([
            {"key":"text","name":"Display name","type":"string","required":false,"default":"factory"},
            {"key":"enabled","name":"Enabled","type":"boolean","required":false,"default":true},
            {"key":"mode","name":"Mode","type":"string","required":false,"choices":["first","second"],"default":"first"},
            {"key":"token","name":"Token","type":"string","required":true,"secret":true},
            {"key":"silent","name":"Never prompt","type":"string","required":false,"prompt":false,"default":"reserved"},
        ]),
    );
    saved_settings(
        &sandbox,
        json!({"text":"before","enabled":false,"mode":"second","token":"old-private-token","unrelated":"retain"}),
    );
    let mut terminal = Terminal::start(sandbox.command(&["plugin", "config", "EXAMPLE", "setup"]));
    terminal.wait_for("Display name");
    terminal.send("\x7f\x7f\x7f\x7f\x7f\x7f\n");
    terminal.wait_for("Enabled");
    terminal.send("\n");
    terminal.wait_for("Mode");
    terminal.send("\n");
    terminal.wait_for("leave blank to keep current");
    terminal.send("\n");
    let (status, text) = terminal.finish();
    assert!(status.success(), "{text}");
    assert!(!text.contains("old-private-token"), "{text}");
    assert!(!text.contains("Never prompt"), "{text}");
    assert_eq!(
        read_settings(&sandbox),
        json!({"text":"","enabled":false,"mode":"second","token":"old-private-token","unrelated":"retain"})
    );
}

#[test]
fn required_secret_validation_retries_without_echoing_the_answer() {
    let sandbox = Sandbox::new();
    installed(
        &sandbox,
        json!([
            {"key":"token","name":"Token","type":"string","required":true,"secret":true,"validation_pattern":r"(?P<word>[a-z]+)-(?P=word)\Z"},
        ]),
    );
    let mut terminal = Terminal::start(sandbox.command(&["plugin", "config", "example", "setup"]));
    terminal.wait_for("Token");
    terminal.send("\n");
    terminal.wait_for("This field is required");
    terminal.send("bad-value\n");
    terminal.wait_for("does not match its validation pattern");
    // Inquire retains invalid input for editing.
    terminal.send(&format!("{}private-private\n", "\x7f".repeat("bad-value".len())));
    let (status, text) = terminal.finish();
    assert!(status.success(), "{text}");
    assert!(!text.contains("private-private"), "{text}");
    assert!(!text.contains("bad-value"), "{text}");
    assert_eq!(read_settings(&sandbox)["token"], "private-private");
}

#[test]
fn cancelling_after_an_answer_preserves_all_configuration_bytes() {
    let sandbox = Sandbox::new();
    installed(
        &sandbox,
        json!([
            {"key":"text","name":"Text","type":"string","required":false},
            {"key":"mode","name":"Mode","type":"string","required":false,"choices":["first","second"]},
        ]),
    );
    let before = saved_settings(&sandbox, json!({"text":"old","mode":"second"}));
    let mut terminal = Terminal::start(sandbox.command(&["plugin", "config", "example", "setup"]));
    terminal.wait_for("Text");
    terminal.send("\x7f\x7f\x7fchanged\n");
    terminal.wait_for("Mode");
    terminal.send("\x03");
    let (status, text) = terminal.finish();
    assert_eq!(status.code(), Some(1), "{text}");
    assert!(!text.contains("Configured example"));
    assert_eq!(fs::read(settings_path(&sandbox)).unwrap(), before);
}

#[test]
fn invalid_existing_choice_is_not_silently_replaced_with_the_first_option() {
    let sandbox = Sandbox::new();
    installed(
        &sandbox,
        json!([
            {"key":"token","name":"Earlier secret","type":"string","required":true,"secret":true},
            {"key":"mode","name":"Mode","type":"string","required":false,"choices":["first","second"]},
        ]),
    );
    let before = saved_settings(&sandbox, json!({"mode":"removed"}));
    let terminal = Terminal::start(sandbox.command(&["plugin", "config", "example", "setup"]));
    let (status, text) = terminal.finish();
    assert!(!status.success(), "{text}");
    assert!(text.contains("not an available choice"), "{text}");
    assert!(!text.contains("Earlier secret"), "{text}");
    assert_eq!(fs::read(settings_path(&sandbox)).unwrap(), before);
}

#[test]
fn optional_blank_pattern_failure_is_reported_after_collecting_the_form() {
    let sandbox = Sandbox::new();
    installed(
        &sandbox,
        json!([
            {"key":"text","name":"Text","type":"string","required":false,"validation_pattern":"[a-z]+"},
            {"key":"flag","name":"Later flag","type":"boolean","required":false},
        ]),
    );
    let before = saved_settings(&sandbox, json!({"unrelated":"retain"}));
    let mut terminal = Terminal::start(sandbox.command(&["plugin", "config", "example", "setup"]));
    terminal.wait_for("Text");
    terminal.send("\n");
    terminal.wait_for("Later flag");
    terminal.send("\n");
    let (status, text) = terminal.finish();
    assert!(!status.success(), "{text}");
    assert!(text.contains("does not match its validation pattern"), "{text}");
    assert_eq!(fs::read(settings_path(&sandbox)).unwrap(), before);
}

#[test]
fn selecting_a_descriptor_default_preserves_an_existing_override_as_upstream_does() {
    let sandbox = Sandbox::new();
    installed(
        &sandbox,
        json!([
            {"key":"text","name":"Text","type":"string","required":false,"default":"factory"},
        ]),
    );
    let before = saved_settings(&sandbox, json!({"text":"override"}));
    let mut terminal = Terminal::start(sandbox.command(&["plugin", "config", "example", "setup"]));
    terminal.wait_for("Text");
    terminal.send(&format!("{}factory\n", "\x7f".repeat("override".len())));
    let (status, text) = terminal.finish();
    assert!(status.success(), "{text}");
    assert_eq!(fs::read(settings_path(&sandbox)).unwrap(), before);
}

#[test]
fn prompt_defaults_follow_upstream_type_specific_fallbacks() {
    let sandbox = Sandbox::new();
    installed(
        &sandbox,
        json!([
            {"key":"flag","name":"Flag","type":"boolean","required":false,"default":true},
            {"key":"text","name":"Text","type":"string","required":false},
        ]),
    );
    saved_settings(&sandbox, json!({"flag":"non-boolean","text":true}));
    let mut terminal = Terminal::start(sandbox.command(&["plugin", "config", "example", "setup"]));
    terminal.wait_for("Flag");
    terminal.send("\n");
    terminal.wait_for("Text");
    terminal.send("x\n");
    let (status, text) = terminal.finish();
    assert!(status.success(), "{text}");
    // The accepted boolean default is omitted, preserving the old stored value.
    assert_eq!(read_settings(&sandbox), json!({"flag":"non-boolean","text":"Truex"}));
}

#[test]
fn setup_without_promptable_settings_does_not_require_a_terminal_or_write_configuration() {
    for settings in [
        json!([]),
        json!([
            {"key":"hidden","name":"Hidden","type":"string","required":false,"prompt":false,"default":"static"},
        ]),
    ] {
        let sandbox = Sandbox::new();
        installed(&sandbox, settings);
        let output = sandbox.run(&["plugin", "config", "example", "setup"]);
        assert_success(&output);
        assert!(String::from_utf8_lossy(&output.stderr).contains("No "));
        assert!(!settings_path(&sandbox).exists());
    }
}

#[test]
fn cancelling_install_configuration_rolls_back_fresh_installs_but_keeps_completed_upgrades() {
    for upgrade in [false, true] {
        let sandbox = Sandbox::new();
        if upgrade {
            installed(&sandbox, json!([]));
        }
        let package = sandbox.path().join("package.zip");
        let mut manifest = identity_manifest("2.0", "https://github.com/example/plugin");
        manifest["plugin"]["settings"] = json!([
            {"key":"token","name":"Token","type":"string","required":true,"secret":true},
        ]);
        archive_manifest(&package, &manifest, &[]);
        let mut terminal = Terminal::start(sandbox.command(&[
            "plugin",
            "install",
            "-U",
            package.to_str().unwrap(),
        ]));
        terminal.wait_for("Token");
        terminal.send("\x03");
        let (status, text) = terminal.finish();
        assert_eq!(status.code(), Some(1), "{text}");
        if upgrade {
            assert_eq!(installed_manifest(&sandbox)["plugin"]["version"], "2.0");
        } else {
            assert!(!sandbox.path().join("idausr/plugins/example").exists(), "{text}");
        }
        assert!(!settings_path(&sandbox).exists());
    }
}
