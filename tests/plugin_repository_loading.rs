//! Source comparisons for filesystem and bundle repository acquisition.

#[path = "plugin_repository_loading/bundle.rs"]
mod bundle;
#[path = "plugin_repository_loading/file_urls.rs"]
#[cfg(unix)]
mod file_urls;
#[path = "plugin_repository_loading/filesystem.rs"]
mod filesystem;
#[path = "plugin_repository_loading/http.rs"]
mod http;
mod support;

use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};
use support::{Sandbox, identity_manifest};
use zip::write::SimpleFileOptions;

fn zip(members: &[(&str, &[u8])]) -> Vec<u8> {
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in members {
        archive.start_file(*name, SimpleFileOptions::default()).unwrap();
        archive.write_all(bytes).unwrap();
    }
    archive.finish().unwrap().into_inner()
}

fn descriptor(name: &str, version: &str) -> Vec<u8> {
    let mut manifest = identity_manifest(version, "https://github.com/example/original");
    manifest["plugin"]["name"] = json!(name);
    serde_json::to_vec(&manifest).unwrap()
}

fn package(name: &str, version: &str) -> Vec<u8> {
    zip(&[("ida-plugin.json", &descriptor(name, version)), ("plugin.py", b"# fixture")])
}

fn bundle(members: &[(&str, &[u8])]) -> Vec<u8> {
    let manifest = serde_json::to_vec(&json!({
        "version": 1, "kind": "hcli-plugin-bundle", "builtAt": "2026-09-15T12:00:00Z",
        "createdBy": {"tool": "hcli", "version": "0.24.0"}, "targetPlatformTags": [],
    }))
    .unwrap();
    let mut entries = vec![("plugin-bundle.json", manifest.as_slice())];
    entries.extend_from_slice(members);
    zip(&entries)
}

fn snapshot(sandbox: &Sandbox, path: &Path, kind: &str) -> Value {
    let output = sandbox.run(&["plugin", "--repo", path.to_str().unwrap(), "repo", "snapshot"]);
    let expected = if output.status.success() {
        json!({"success": true, "snapshot": serde_json::from_slice::<Value>(&output.stdout).unwrap()})
    } else {
        assert!(output.stdout.is_empty());
        json!({"success": false})
    };
    if let Some(actual) = source(path, kind) {
        assert_eq!(actual, expected, "{}", String::from_utf8_lossy(&output.stderr));
    }
    expected
}

fn source(path: &Path, kind: &str) -> Option<Value> {
    let python = std::env::var_os("HY_TEST_BUNDLE_ORACLE_PYTHON")?;
    let source = std::env::var_os("HY_TEST_HCLI_SOURCE").map(PathBuf::from).unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli")
    });
    let output = Command::new(python)
        .args(["-I", "-B", "-c", include_str!("plugin_repository_loading/reference.py")])
        .arg(source)
        .arg(path)
        .arg(kind)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    Some(serde_json::from_slice(&output.stdout).unwrap())
}
