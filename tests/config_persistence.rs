//! Flat-key migration and durable IDA/source configuration changes.
#![cfg(unix)]

mod support;

use serde_json::{Value, json};
use std::fs;
use support::*;

#[test]
fn removed_instances_cannot_reappear_from_legacy_nested_configuration() {
    let sandbox = Sandbox::new();
    let path = sandbox.config_path();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        serde_json::to_vec(&json!({
            "ke": {"ida": {"instances": {"legacy": "/fixture/ida"}, "default": "legacy"}},
            "unrelated": 42,
        }))
        .unwrap(),
    )
    .unwrap();
    assert_success(&sandbox.run(&["ke", "ida", "remove", "--all"]));
    let saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved["ida.instances"], json!({}));
    assert!(saved.get("ida.default").is_none());
    assert!(saved["ke"]["ida"].get("instances").is_none());
    assert!(saved["ke"]["ida"].get("default").is_none());
    assert_eq!(saved["unrelated"], 42);
    let output = sandbox.run(&["ke", "ida", "list"]);
    assert_success(&output);
    assert!(!String::from_utf8_lossy(&output.stderr).contains("legacy"));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("legacy"));
}

#[test]
fn source_configuration_failure_cannot_report_success() {
    let sandbox = Sandbox::new();
    let directory = sandbox.config_path().parent().unwrap().to_owned();
    fs::create_dir_all(directory.parent().unwrap()).unwrap();
    fs::write(&directory, b"existing file prevents config directory creation").unwrap();
    let output = sandbox.run(&["ke", "source", "add", "fixture", sandbox.path().to_str().unwrap()]);
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("I/O error"));
    assert!(!error.contains("Added source"));
    assert_eq!(fs::read(directory).unwrap(), b"existing file prevents config directory creation");
}
