//! Snapshot validation and serialization through the CLI and upstream reader.
#![cfg(unix)]

mod support;

use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::Command;

use serde_json::{Value, json};

use support::http::{Response, Server};
use support::{
    Sandbox, archive_manifest, assert_success, fake_python, identity_manifest, repository_snapshot,
};

#[test]
fn malformed_snapshot_envelopes_fail_before_archive_fetch_or_pip() {
    let sandbox = Sandbox::new();
    let package = sandbox.path().join("plugin.zip");
    let metadata = identity_manifest("1", "https://github.com/example/snapshot");
    archive_manifest(&package, &metadata, &[]);
    let server = Server::start(|_, _| Response::missing());
    let mut base = repository_snapshot(&package, &metadata);
    base["plugins"][0]["versions"]["1"][0]["url"] = json!(format!("{}/archive.zip", server.url));
    let python = sandbox.path().join("python");
    let calls = sandbox.path().join("pip-calls");
    fake_python(&python);
    for (parent, field, value) in [
        ("", "plugins", None),
        ("", "version", Some(json!(2))),
        ("/plugins/0", "host", None),
        ("/plugins/0/versions/1/0", "sha256", None),
        ("/plugins/0/versions/1/0", "sha256", Some(Value::Null)),
        ("/plugins/0/versions/1/0", "sha256", Some(json!(42))),
        ("/plugins/0/versions/1/0/metadata", "IDAMetadataDescriptorVersion", None),
        ("/plugins/0/versions/1/0/metadata", "IDAMetadataDescriptorVersion", Some(Value::Null)),
        ("/plugins/0/versions/1/0/metadata", "IDAMetadataDescriptorVersion", Some(json!(2))),
        ("/plugins/0/versions/1/0/metadata", "$schema", Some(json!(false))),
    ] {
        let mut document = base.clone();
        let target = document.pointer_mut(parent).unwrap().as_object_mut().unwrap();
        match value {
            Some(value) => {
                target.insert(field.into(), value);
            }
            None => {
                target.remove(field);
            }
        }
        let repository = sandbox.path().join("repository.json");
        fs::write(&repository, serde_json::to_vec(&document).unwrap()).unwrap();
        compare_source(&repository, "validate", json!({"valid": false}));
        let output = sandbox.path().join("bundle.zip");
        let result = bundle_command(&sandbox, &repository, &output)
            .env("HCLI_CURRENT_IDA_PYTHON_EXE", &python)
            .env("HY_TEST_PIP_ARGUMENTS", &calls)
            .output()
            .unwrap();
        assert!(!result.status.success(), "{parent}/{field}: {result:?}");
        assert!(server.requests().is_empty());
        assert!(!output.exists());
        assert!(!calls.exists());
    }
}

#[test]
fn snapshot_version_defaults_and_literal_coercions_round_trip() {
    let sandbox = Sandbox::new();
    for version in [None, Some(json!(1)), Some(json!(1.0)), Some(json!(true))] {
        let mut document = json!({"plugins": []});
        if let Some(version) = version {
            document["version"] = version;
        }
        let repository = sandbox.path().join("repository.json");
        fs::write(&repository, serde_json::to_vec(&document).unwrap()).unwrap();
        compare_source(&repository, "validate", json!({"valid": true}));
        let result = snapshot(&sandbox, &repository);
        assert_eq!(result, json!({"version": 1, "plugins": []}));
    }
}

#[test]
fn invalid_version_keys_fail_before_fetching_a_valid_matching_archive() {
    let sandbox = Sandbox::new();
    let package = sandbox.path().join("plugin.zip");
    let metadata = identity_manifest("1", "https://github.com/example/snapshot");
    archive_manifest(&package, &metadata, &[]);
    let server = Server::start(|_, _| Response::missing());
    let mut base = repository_snapshot(&package, &metadata);
    base["plugins"][0]["versions"]["1"][0]["url"] = json!(format!("{}/plugin.zip", server.url));
    for first in [false, true] {
        let mut document = base.clone();
        let versions = document["plugins"][0]["versions"].as_object_mut().unwrap();
        let valid = versions.remove("1").unwrap();
        if first {
            versions.insert("bad".into(), json!([]));
        }
        versions.insert("1".into(), valid);
        if !first {
            versions.insert("bad".into(), json!([]));
        }
        let repository = sandbox.path().join("repository.json");
        fs::write(&repository, serde_json::to_vec(&document).unwrap()).unwrap();
        compare_source(&repository, "select", json!({"valid": true, "selection_error": true}));
        let output = sandbox.path().join("bundle.zip");
        let result = bundle_command(&sandbox, &repository, &output).output().unwrap();
        assert!(!result.status.success());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("invalid plugin repository version"),
            "{result:?}"
        );
        assert!(server.requests().is_empty());
        assert!(!output.exists());
    }
}

#[test]
fn exported_local_snapshots_include_complete_descriptors() {
    let sandbox = Sandbox::new();
    let directory = sandbox.path().join("repository");
    fs::create_dir(&directory).unwrap();
    let mut descriptor = identity_manifest("1", "https://github.com/example/snapshot");
    descriptor["$schema"] = json!("https://example.test/schema.json");
    archive_manifest(&directory.join("plugin.zip"), &descriptor, &[]);
    let document = snapshot(&sandbox, &directory);
    let metadata = &document["plugins"][0]["versions"]["1"][0]["metadata"];
    assert_eq!(metadata["IDAMetadataDescriptorVersion"], 1);
    assert!(metadata.get("$schema").is_none());
    let output = sandbox.path().join("snapshot.json");
    fs::write(&output, serde_json::to_vec(&document).unwrap()).unwrap();
    compare_source(&output, "validate", json!({"valid": true}));
}

#[test]
fn version_ties_follow_input_order_and_sorted_snapshot_round_trips() {
    let sandbox = Sandbox::new();
    let metadata = identity_manifest("1", "https://github.com/example/snapshot");
    let first = sandbox.path().join("first.zip");
    let second = sandbox.path().join("second.zip");
    archive_manifest(&first, &metadata, &[("first", b"first")]);
    archive_manifest(&second, &metadata, &[("second", b"second")]);
    let locations = [
        repository_snapshot(&first, &metadata)["plugins"][0]["versions"]["1"].clone(),
        repository_snapshot(&second, &metadata)["plugins"][0]["versions"]["1"].clone(),
    ];
    for order in [[0, 1], [1, 0]] {
        let keys = ["1.0", "1"];
        let versions = order
            .iter()
            .map(|index| {
                format!("{:?}:{}", keys[*index], serde_json::to_string(&locations[*index]).unwrap())
            })
            .collect::<Vec<_>>()
            .join(",");
        let document = format!(
            "{{\"plugins\":[{{\"name\":\"example\",\"host\":\"https://github.com/example/snapshot\",\"versions\":{{{versions}}}}}]}}",
        );
        let repository = sandbox.path().join("repository.json");
        fs::write(&repository, document).unwrap();
        compare_source(
            &repository,
            "select",
            json!({"valid": true, "selected": locations[order[0]][0]["url"]}),
        );
        let output = sandbox.path().join("bundle.zip");
        assert_success(&bundle_command(&sandbox, &repository, &output).output().unwrap());
        let mut bundle = zip::ZipArchive::new(fs::File::open(output).unwrap()).unwrap();
        let mut bytes = Vec::new();
        bundle.by_name("plugins/example-1.zip").unwrap().read_to_end(&mut bytes).unwrap();
        let selected = if order[0] == 0 {
            &first
        } else {
            &second
        };
        assert_eq!(bytes, fs::read(selected).unwrap());
        let exported = snapshot(&sandbox, &repository);
        let versions = exported["plugins"][0]["versions"].as_object().unwrap();
        assert_eq!(versions.keys().map(String::as_str).collect::<Vec<_>>(), ["1", "1.0"]);
        let path = sandbox.path().join("exported.json");
        fs::write(&path, serde_json::to_vec(&exported).unwrap()).unwrap();
        compare_source(&path, "select", json!({"valid": true, "selected": locations[1][0]["url"]}));
    }
}

#[test]
fn installation_uses_case_sensitive_hash_verification() {
    let sandbox = Sandbox::new();
    let metadata = identity_manifest("1", "https://github.com/example/snapshot");
    let package = sandbox.path().join("plugin.zip");
    archive_manifest(&package, &metadata, &[]);
    let mut document = repository_snapshot(&package, &metadata);
    let checksum = &mut document["plugins"][0]["versions"]["1"][0]["sha256"];
    let uppercase = checksum.as_str().unwrap().to_uppercase();
    assert_ne!(checksum.as_str().unwrap(), uppercase);
    *checksum = json!(uppercase);
    let repository = sandbox.path().join("repository.json");
    fs::write(&repository, serde_json::to_vec(&document).unwrap()).unwrap();
    let result = sandbox.run(&[
        "plugin",
        "--repo",
        repository.to_str().unwrap(),
        "--no-python-environment-check",
        "install",
        "example==1",
    ]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("hash mismatch"), "{result:?}");
    assert!(!sandbox.path().join("idausr/plugins/example").exists());
}

#[test]
fn snapshot_text_matches_source_key_order_escaping_and_numbers() {
    let sandbox = Sandbox::new();
    let package = sandbox.path().join("plugin.zip");
    let metadata = identity_manifest("1", "https://github.com/example/snapshot");
    archive_manifest(&package, &metadata, &[]);
    let base = repository_snapshot(&package, &metadata);
    for extra in [
        json!("quotes \" and \\; 日本語 🧠\n\t\u{7f}"),
        serde_json::from_str("[0,-0,-0.0,1.0,1e-7,1e-4,1e15,1e16,1e999,-1e999]").unwrap(),
        json!({"z": [[], {}, {"b": 2, "a": 1}], "a": "é", "🧠": null}),
    ] {
        let mut document = base.clone();
        document["plugins"][0]["versions"]["1"][0]["metadata"]["plugin"]["extraField"] = extra;
        let repository = sandbox.path().join("repository.json");
        fs::write(&repository, serde_json::to_vec(&document).unwrap()).unwrap();
        let result =
            sandbox.run(&["plugin", "--repo", repository.to_str().unwrap(), "repo", "snapshot"]);
        assert_success(&result);
        assert!(result.stdout.is_ascii());
        assert!(result.stdout.starts_with(b"{\n    \"plugins\": [\n"));
        assert!(result.stdout.ends_with(b"\n}\n"));
        compare_source(
            &repository,
            "render",
            json!({
                "status": 0, "stdout": String::from_utf8(result.stdout).unwrap(),
            }),
        );
    }
}

fn snapshot(sandbox: &Sandbox, repository: &Path) -> Value {
    let result =
        sandbox.run(&["plugin", "--repo", repository.to_str().unwrap(), "repo", "snapshot"]);
    assert_success(&result);
    serde_json::from_slice(&result.stdout).unwrap()
}

fn bundle_command(sandbox: &Sandbox, repository: &Path, output: &Path) -> Command {
    sandbox.command(&[
        "plugin",
        "--repo",
        repository.to_str().unwrap(),
        "bundle",
        "create",
        "--path",
        output.to_str().unwrap(),
        "--target",
        "linux-x86_64-cp312",
        "example==1",
    ])
}

fn compare_source(path: &Path, mode: &str, expected: Value) {
    let Some(python) = std::env::var_os("HY_TEST_BUNDLE_ORACLE_PYTHON") else {
        return;
    };
    let source =
        std::env::var_os("HY_TEST_HCLI_SOURCE").map(std::path::PathBuf::from).unwrap_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli"),
        );
    let output = Command::new(python)
        .args(["-I", "-B", "-c", include_str!("plugin_snapshot/reference.py")])
        .arg(source)
        .arg(path)
        .arg(mode)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(serde_json::from_slice::<Value>(&output.stdout).unwrap(), expected);
}
