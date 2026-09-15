use std::io::{Cursor, Write};

use serde_json::json;
use zip::write::SimpleFileOptions;

fn zip(members: &[(&str, &[u8])]) -> Vec<u8> {
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in members {
        archive.start_file(*name, SimpleFileOptions::default()).unwrap();
        archive.write_all(bytes).unwrap();
    }
    archive.finish().unwrap().into_inner()
}

fn package(version: &str) -> Vec<u8> {
    let manifest = serde_json::to_vec(&json!({
        "IDAMetadataDescriptorVersion": 1,
        "plugin": {"name": "example", "version": version, "entryPoint": "plugin.py",
            "urls": {"repository": "https://github.com/example/original"},
            "authors": [{"email": "author@example.test"}]},
    }))
    .unwrap();
    zip(&[("ida-plugin.json", &manifest), ("plugin.py", b"# fixture")])
}

fn bundle(package: &[u8]) -> Vec<u8> {
    let manifest = serde_json::to_vec(&json!({
        "version": 1, "kind": "hcli-plugin-bundle", "builtAt": "2026-09-15T12:00:00Z",
        "createdBy": {"tool": "hcli", "version": "0.24.0"}, "targetPlatformTags": [{
            "id": "fixture", "idaPlatform": "windows-x86_64", "pythonVersion": "3.12",
            "implementation": "cp", "abis": [], "pipPlatformTags": [], "wheelhouse": "wheels",
        }],
    }))
    .unwrap();
    zip(&[
        ("plugin-bundle.json", &manifest),
        ("plugins/example.zip", package),
        ("wheels/fixture.whl", package),
    ])
}

#[tokio::test]
async fn bundle_fetch_keeps_the_open_file_and_still_verifies_the_selected_hash() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("bundle.zip");
    let original = package("1");
    std::fs::write(&path, bundle(&original)).unwrap();
    let loaded = super::load(path.to_str().unwrap(), true).await.unwrap();
    let mut location = loaded.snapshot.plugins[0].versions["1"][0].clone();

    std::fs::rename(&path, directory.path().join("previous.zip")).unwrap();
    std::fs::write(&path, bundle(&package("2"))).unwrap();
    assert_eq!(loaded.fetch_verified(&location).await.unwrap(), original);
    let replacement = super::load(path.to_str().unwrap(), true).await.unwrap();
    assert!(replacement.snapshot.plugins[0].versions.contains_key("2"));

    for (repository, name, expected) in [
        (&loaded, "original-wheels", original.as_slice()),
        (&replacement, "replacement-wheels", package("2").as_slice()),
    ] {
        let reader = repository.bundle_reader().unwrap();
        let target = &reader.manifest().target_platform_tags[0];
        let destination = directory.path().join(name);
        reader.extract_wheelhouse(target, &destination).unwrap();
        assert_eq!(std::fs::read(destination.join("fixture.whl")).unwrap(), expected);
    }

    location.sha256 = "invalid".into();
    let error = loaded.fetch_verified(&location).await.unwrap_err().to_string();
    assert!(error.contains("hash mismatch: expected invalid"), "{error}");
    assert!(error.contains("hcli-bundle:plugins/example.zip"), "{error}");
}
