//! Canonical installed records and the separate status inventory for older formats.

mod support;

use std::fs;
use std::path::PathBuf;

use serde_json::{Value, json};
use support::*;

fn descriptor(sandbox: &Sandbox, directory: &str, manifest: &Value) -> PathBuf {
    let path = sandbox.path().join("idausr/plugins").join(directory);
    fs::create_dir_all(&path).unwrap();
    fs::write(path.join("ida-plugin.json"), serde_json::to_vec(manifest).unwrap()).unwrap();
    fs::write(path.join("plugin.py"), "# inventory fixture\n").unwrap();
    path
}

fn manifest(name: &str) -> Value {
    let mut manifest = identity_manifest("1.0", "https://github.com/example/plugins");
    manifest["plugin"]["name"] = json!(name);
    manifest
}

fn status(sandbox: &Sandbox, names: &[&str]) -> (bool, Vec<Value>) {
    let mut args = vec!["plugin", "status", "--skip-upgrade-check", "--json"];
    args.extend_from_slice(names);
    let output = sandbox.run(&args);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!("invalid status JSON: {error}; stderr: {}", String::from_utf8_lossy(&output.stderr))
    });
    (output.status.success(), report["plugins"].as_array().unwrap().clone())
}

#[test]
fn status_distinguishes_managed_minimal_legacy_and_broken_entries() {
    let sandbox = Sandbox::new();
    descriptor(&sandbox, "managed", &manifest("managed"));
    descriptor(
        &sandbox,
        "older-layout",
        &json!({"IDAMetadataDescriptorVersion": 1, "plugin": {"name": "Old Plugin"}}),
    );
    descriptor(
        &sandbox,
        "older-version",
        &json!({"IDAMetadataDescriptorVersion": true, "plugin": {"name": "Old Version", "version": ""}}),
    );
    // Modern metadata that fails only file or name checks is not a legacy format.
    let missing_entry = descriptor(&sandbox, "missing-entry", &manifest("missing-entry"));
    fs::remove_file(missing_entry.join("plugin.py")).unwrap();
    descriptor(&sandbox, "wrong-name", &manifest("different-name"));
    descriptor(&sandbox, "CaseOnly", &manifest("caseonly"));
    descriptor(&sandbox, "flat", &json!({"name": "flat", "version": "1"}));
    descriptor(
        &sandbox,
        "wrong-wrapper",
        &json!({"IDAMetadataDescriptorVersion": 2, "plugin": {"name": "wrong-wrapper"}}),
    );
    let malformed = descriptor(&sandbox, "malformed", &manifest("malformed"));
    fs::write(malformed.join("ida-plugin.json"), "{invalid").unwrap();
    let root = sandbox.path().join("idausr/plugins");
    fs::create_dir(root.join("empty")).unwrap();
    for name in ["legacy.py", "legacy.so", "legacy.dll", "legacy.dylib", "ignored.PY", "notes.txt"]
    {
        fs::write(root.join(name), "# fixture").unwrap();
    }

    let (success, entries) = status(&sandbox, &[]);
    assert!(success);
    assert_eq!(entries.len(), 7, "{entries:?}");
    assert_eq!(entries[0]["name"], "managed");
    assert_eq!(entries[0]["kind"], "installed");
    for (name, path) in [("Old Plugin", "older-layout/"), ("Old Version", "older-version/")] {
        let entry = entries.iter().find(|entry| entry["name"] == name).unwrap();
        assert_eq!(entry["kind"], "incompatible");
        assert_eq!(entry["version"], Value::Null);
        assert_eq!(entry["path"], path);
    }
    for name in ["legacy.py", "legacy.so", "legacy.dll", "legacy.dylib"] {
        let entry = entries.iter().find(|entry| entry["name"] == name).unwrap();
        assert_eq!(entry["kind"], "legacy");
        assert_eq!(entry["path"], name);
    }

    let (success, entries) = status(
        &sandbox,
        &["MANAGED", "Old Plugin", "legacy.py", "wrong-name", "CaseOnly", "missing-entry"],
    );
    assert!(!success);
    assert_eq!(entries[0]["name"], "managed");
    for entry in &entries[1..] {
        assert_eq!(entry["installed"], false);
        assert!(entry.get("kind").is_none());
    }
}

#[test]
fn named_status_preserves_requested_order_and_repeated_names() {
    let sandbox = Sandbox::new();
    descriptor(&sandbox, "alpha", &manifest("alpha"));
    descriptor(&sandbox, "zeta", &manifest("zeta"));
    let (success, entries) = status(&sandbox, &["ZETA", "alpha", "zeta"]);
    assert!(success);
    let names: Vec<_> = entries.iter().map(|entry| entry["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["zeta", "alpha", "zeta"]);
}

#[test]
fn replacement_publishes_the_exact_descriptor_name_after_case_changes() {
    let sandbox = Sandbox::new();
    descriptor(&sandbox, "example", &manifest("example"));
    let package = sandbox.path().join("case-change.zip");
    let mut candidate = manifest("Example");
    candidate["plugin"]["version"] = json!("2.0");
    archive_manifest(&package, &candidate, &[]);
    assert_success(&sandbox.run(&["plugin", "install", "-U", package.to_str().unwrap()]));
    let names: Vec<_> = fs::read_dir(sandbox.path().join("idausr/plugins"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(names, ["Example"]);
    let (success, entries) = status(&sandbox, &["EXAMPLE"]);
    assert!(success);
    assert_eq!(entries[0]["name"], "Example");
    assert_eq!(entries[0]["version"], "2.0");
}

#[test]
#[cfg(unix)]
fn editable_source_inside_replaced_directory_is_preserved() {
    let sandbox = Sandbox::new();
    let old = descriptor(&sandbox, "example", &manifest("example"));
    let source = descriptor(&sandbox, "example/source", &manifest("example"));
    let output = sandbox.run(&["plugin", "install", "--editable", source.to_str().unwrap()]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("inside the installation"));
    assert!(source.join("ida-plugin.json").is_file());
    assert!(source.join("plugin.py").is_file());
    assert!(old.join("ida-plugin.json").is_file());
    assert!(!old.is_symlink());
}

#[test]
fn broken_installation_rejects_managed_operations_and_remains_removable() {
    for failure in ["missing-entry", "name-mismatch", "case-mismatch"] {
        let sandbox = Sandbox::new();
        let mut installed = manifest("example");
        match failure {
            "name-mismatch" => installed["plugin"]["name"] = json!("another"),
            "case-mismatch" => installed["plugin"]["name"] = json!("Example"),
            _ => {}
        }
        let path = descriptor(&sandbox, "example", &installed);
        if failure == "missing-entry" {
            fs::remove_file(path.join("plugin.py")).unwrap();
        }
        let before = fs::read(path.join("ida-plugin.json")).unwrap();
        for args in
            [vec!["plugin", "config", "EXAMPLE", "list"], vec!["plugin", "upgrade", "EXAMPLE"]]
        {
            let output = sandbox.run(&args);
            assert!(!output.status.success(), "{failure}");
            assert!(String::from_utf8_lossy(&output.stderr).contains("not installed"));
        }
        let candidate = sandbox.path().join("candidate.zip");
        archive_manifest(&candidate, &manifest("example"), &[]);
        let output = sandbox.run(&["plugin", "install", "--force", candidate.to_str().unwrap()]);
        assert!(!output.status.success(), "{failure}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("hy plugin uninstall example"));
        assert_eq!(fs::read(path.join("ida-plugin.json")).unwrap(), before);
        assert_success(&sandbox.run(&["plugin", "uninstall", "EXAMPLE"]));
        assert!(!path.exists());
    }
}

#[test]
#[cfg(unix)]
fn dependency_preflight_uses_only_validated_installed_records() {
    let sandbox = Sandbox::new();
    let mut retained = manifest("retained");
    retained["plugin"]["pythonDependencies"] = json!(["retained==1"]);
    descriptor(&sandbox, "retained", &retained);
    for (directory, name) in [("broken", "broken"), ("wrong-name", "different-name")] {
        let mut invalid = manifest(name);
        invalid["plugin"]["pythonDependencies"] = json!(["must-not-resolve==99"]);
        let path = descriptor(&sandbox, directory, &invalid);
        if directory == "broken" {
            fs::remove_file(path.join("plugin.py")).unwrap();
        }
    }
    descriptor(
        &sandbox,
        "minimal",
        &json!({"IDAMetadataDescriptorVersion": 1, "plugin": {"name": "minimal", "version": "1"}}),
    );
    let package = sandbox.path().join("example.zip");
    archive_with_dependencies(&package, "1", &[], &["candidate==2"]);
    let interpreter = sandbox.path().join("python");
    fake_python(&interpreter);
    let arguments = sandbox.path().join("arguments");
    assert_success(&sandbox.run_with_env(
        &["plugin", "--no-python-environment-check", "install", package.to_str().unwrap()],
        &[("HCLI_CURRENT_IDA_PYTHON_EXE", &interpreter), ("HY_TEST_PIP_ARGUMENTS", &arguments)],
    ));
    let calls = fs::read_to_string(arguments).unwrap();
    assert_eq!(calls.matches("retained==1").count(), 2);
    assert_eq!(calls.matches("candidate==2").count(), 2);
    assert!(!calls.contains("must-not-resolve"));
}

#[test]
fn removal_accepts_legacy_filenames_without_accepting_path_traversal() {
    let sandbox = Sandbox::new();
    let root = sandbox.path().join("idausr/plugins");
    fs::create_dir_all(&root).unwrap();
    for name in ["Old Plugin.py", "broken.directory", ".hidden-remnant", "café.py"] {
        fs::write(root.join(name), "retained until explicit removal").unwrap();
        assert_success(&sandbox.run(&["plugin", "uninstall", &name.to_uppercase()]));
        assert!(!root.join(name).exists());
    }
    let outside = sandbox.path().join("idausr/keep");
    fs::write(&outside, "retain").unwrap();
    for name in ["..", "../keep", "..\\keep", "/keep", "bad\nname"] {
        assert!(!sandbox.run(&["plugin", "uninstall", name]).status.success());
    }
    assert_eq!(fs::read_to_string(outside).unwrap(), "retain");
}
