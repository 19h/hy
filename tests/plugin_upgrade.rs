//! Repository identity, archive selection, and upgrade regressions.

mod support;

use std::fs;

use serde_json::json;
use support::*;

#[test]
fn upgrade_searches_all_repositories_using_installed_identity() {
    let sandbox = Sandbox::new();
    let original = sandbox.path().join("original.zip");
    let host = "https://github.com/example/original";
    archive_manifest(&original, &identity_manifest("1.0", host), &[]);
    assert_success(&sandbox.run(&["plugin", "install", original.to_str().unwrap()]));

    for name in ["community", "hexrays"] {
        assert_success(&sandbox.run(&["plugin", "repo", "remove", name]));
    }
    for (name, version, identity) in
        [("original", "2.0", host), ("unrelated", "99.0", "https://github.com/example/unrelated")]
    {
        let repository = sandbox.path().join(name);
        fs::create_dir(&repository).unwrap();
        archive_manifest(
            &repository.join("example.zip"),
            &identity_manifest(version, identity),
            &[],
        );
        add_repository(&sandbox, name, &repository);
    }
    assert_success(&sandbox.run(&["plugin", "repo", "set-default", "unrelated"]));
    assert_success(&sandbox.run(&["plugin", "upgrade", "EXAMPLE"]));
    assert_eq!(installed_manifest(&sandbox)["plugin"]["version"], "2.0");

    let duplicate = sandbox.path().join("duplicate");
    fs::create_dir(&duplicate).unwrap();
    archive_manifest(&duplicate.join("example.zip"), &identity_manifest("3.0", host), &[]);
    add_repository(&sandbox, "duplicate", &duplicate);
    let status = sandbox.run(&["plugin", "status", "--json", "EXAMPLE"]);
    assert_success(&status);
    let report: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(report["plugins"][0]["in_repository"], false);
    assert_eq!(report["plugins"][0]["upgradable_to"], serde_json::Value::Null);
    let ambiguous = sandbox.run(&["plugin", "upgrade", "example"]);
    assert!(!ambiguous.status.success());
    assert!(String::from_utf8_lossy(&ambiguous.stderr).contains("multiple repositories"));
    assert_success(&sandbox.run(&["plugin", "repo", "remove", "duplicate"]));

    let unchanged = sandbox.run(&["plugin", "upgrade", "example"]);
    assert!(!unchanged.status.success());
    assert!(String::from_utf8_lossy(&unchanged.stderr).contains("not newer"));

    let foreign = sandbox.run(&["plugin", "upgrade", "unrelated/example"]);
    assert!(!foreign.status.success());
    assert_eq!(installed_manifest(&sandbox)["plugin"]["version"], "2.0");
}

#[test]
fn install_upgrade_keeps_equal_or_newer_files_and_settings() {
    let sandbox = Sandbox::new();
    let original = sandbox.path().join("original.zip");
    archive(&original, "2.0", &[("package/sentinel", b"original")]);
    assert_success(&sandbox.run(&["plugin", "install", original.to_str().unwrap()]));
    assert_success(&sandbox.run(&["plugin", "config", "example", "set", "enabled", "true"]));
    let configuration = sandbox.path().join("idausr/ida-config.json");
    let before = fs::read(&configuration).unwrap();

    for version in ["2.0", "1.0"] {
        let candidate = sandbox.path().join(format!("candidate-{version}.zip"));
        archive_with_dependencies(&candidate, version, &[], &["must-not-be-installed"]);
        assert_success(&sandbox.run(&["plugin", "install", "-U", candidate.to_str().unwrap()]));
        assert_eq!(installed_manifest(&sandbox)["plugin"]["version"], "2.0");
        assert_eq!(fs::read(&configuration).unwrap(), before);
        assert_eq!(
            fs::read(sandbox.path().join("idausr/plugins/example/sentinel")).unwrap(),
            b"original"
        );
    }

    let invalid = sandbox.path().join("invalid-version.zip");
    archive(&invalid, "invalid", &[]);
    let output = sandbox.run(&["plugin", "install", "-U", invalid.to_str().unwrap()]);
    assert!(!output.status.success());
    // Invalid versions are rejected while selecting archive descriptors.
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("ida-plugin.json not found in archive")
    );
    assert_eq!(installed_manifest(&sandbox)["plugin"]["version"], "2.0");
    assert_eq!(fs::read(&configuration).unwrap(), before);
}

#[test]
fn installation_uses_the_selected_archives_version_and_host() {
    for field in ["name", "case", "version", "host"] {
        let sandbox = Sandbox::new();
        let host = "https://github.com/example/original";
        let mut manifest = identity_manifest("1.0", host);
        match field {
            "name" => manifest["plugin"]["name"] = json!("another"),
            "case" => manifest["plugin"]["name"] = json!("EXAMPLE"),
            "version" => manifest["plugin"]["version"] = json!("2.0"),
            "host" => {
                manifest["plugin"]["urls"]["repository"] =
                    json!("https://github.com/example/another")
            }
            _ => unreachable!(),
        }
        let package = sandbox.path().join("candidate.zip");
        archive_manifest(&package, &manifest, &[]);
        let snapshot = repository_snapshot(&package, &identity_manifest("1.0", host));
        let repository = sandbox.path().join("repository.json");
        fs::write(&repository, serde_json::to_vec(&snapshot).unwrap()).unwrap();
        let output =
            sandbox.run(&["plugin", "--repo", repository.to_str().unwrap(), "install", "example"]);
        assert_archive_selection(
            &sandbox,
            &repository,
            &output,
            &manifest,
            matches!(field, "name" | "case"),
        );
    }
}

#[test]
fn repository_install_selects_only_the_requested_subtree() {
    let sandbox = Sandbox::new();
    let host = "https://github.com/example/original";
    let manifest = identity_manifest("1.0", host);
    let mut other = manifest.clone();
    other["plugin"]["name"] = json!("another");
    let other = serde_json::to_vec(&other).unwrap();
    let package = sandbox.path().join("multiple.zip");
    archive_manifest(
        &package,
        &manifest,
        &[("other/ida-plugin.json", &other), ("other/plugin.py", b"# another plugin")],
    );
    assert!(!sandbox.run(&["plugin", "install", package.to_str().unwrap()]).status.success());

    let repository = sandbox.path().join("repository.json");
    let snapshot = repository_snapshot(&package, &manifest);
    fs::write(&repository, serde_json::to_vec(&snapshot).unwrap()).unwrap();
    assert_success(&sandbox.run(&[
        "plugin",
        "--repo",
        repository.to_str().unwrap(),
        "install",
        "example",
    ]));
    assert!(sandbox.path().join("idausr/plugins/example/plugin.py").is_file());
    assert!(!sandbox.path().join("idausr/plugins/example/other").exists());
    assert!(!sandbox.path().join("idausr/plugins/another").exists());
}

#[test]
fn named_repositories_cannot_claim_hexrays_identities() {
    let sandbox = Sandbox::new();
    let repository = sandbox.path().join("repository");
    fs::create_dir(&repository).unwrap();
    archive_manifest(
        &repository.join("example.zip"),
        &identity_manifest("1.0", "https://plugins.hex-rays.com/example/plugin"),
        &[],
    );
    add_repository(&sandbox, "custom", &repository);
    let output = sandbox.run(&["plugin", "install", "custom/example"]);
    assert!(!output.status.success());
    assert!(!sandbox.path().join("idausr/plugins/example").exists());

    assert_success(&sandbox.run(&[
        "plugin",
        "--repo",
        repository.to_str().unwrap(),
        "install",
        "example",
    ]));
}

#[test]
fn repository_install_uses_the_first_exact_name_descriptor() {
    let sandbox = Sandbox::new();
    let host = "https://github.com/example/original";
    let manifest = identity_manifest("1.0", host);
    let duplicate = serde_json::to_vec(&identity_manifest("99.0", host)).unwrap();
    let package = sandbox.path().join("duplicate.zip");
    archive_manifest(
        &package,
        &manifest,
        &[("other/ida-plugin.json", &duplicate), ("other/plugin.py", b"# duplicate")],
    );
    let repository = sandbox.path().join("repository.json");
    fs::write(&repository, serde_json::to_vec(&repository_snapshot(&package, &manifest)).unwrap())
        .unwrap();
    let output =
        sandbox.run(&["plugin", "--repo", repository.to_str().unwrap(), "install", "example"]);
    assert_archive_selection(&sandbox, &repository, &output, &manifest, false);
    assert!(!sandbox.path().join("idausr/plugins/example/other").exists());
}

#[test]
fn location_descriptor_supplies_the_archive_name_only() {
    for field in ["name", "case", "version", "host"] {
        let sandbox = Sandbox::new();
        let package = sandbox.path().join("example.zip");
        let manifest = identity_manifest("1.0", "https://github.com/example/original");
        archive_manifest(&package, &manifest, &[]);
        let mut snapshot = repository_snapshot(&package, &manifest);
        let descriptor = &mut snapshot["plugins"][0]["versions"]["1.0"][0]["metadata"]["plugin"];
        match field {
            "name" => descriptor["name"] = json!("another"),
            "case" => descriptor["name"] = json!("EXAMPLE"),
            "version" => descriptor["version"] = json!("2.0"),
            "host" => {
                descriptor["urls"]["repository"] = json!("https://github.com/example/another")
            }
            _ => unreachable!(),
        }
        let repository = sandbox.path().join("repository.json");
        fs::write(&repository, serde_json::to_vec(&snapshot).unwrap()).unwrap();
        let output =
            sandbox.run(&["plugin", "--repo", repository.to_str().unwrap(), "install", "example"]);
        assert_archive_selection(
            &sandbox,
            &repository,
            &output,
            &manifest,
            matches!(field, "name" | "case"),
        );
    }
}

fn assert_archive_selection(
    sandbox: &Sandbox,
    repository: &std::path::Path,
    output: &std::process::Output,
    manifest: &serde_json::Value,
    missing_name: bool,
) {
    let expected = if missing_name {
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("not found"));
        assert!(!sandbox.path().join("idausr/plugins/example").exists());
        assert!(!sandbox.path().join("idausr/plugins/another").exists());
        json!({"missing": true})
    } else {
        assert_success(output);
        let installed = installed_manifest(sandbox);
        for field in ["name", "version", "urls"] {
            assert_eq!(installed["plugin"][field], manifest["plugin"][field]);
        }
        json!({
            "name": installed["plugin"]["name"],
            "version": installed["plugin"]["version"],
            "host": installed["plugin"]["urls"]["repository"],
        })
    };
    let Some(python) = std::env::var_os("HY_TEST_BUNDLE_ORACLE_PYTHON") else {
        return;
    };
    let source =
        std::env::var_os("HY_TEST_HCLI_SOURCE").map(std::path::PathBuf::from).unwrap_or_else(
            || std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli"),
        );
    let result = std::process::Command::new(python)
        .args(["-I", "-B", "-c", include_str!("plugin_upgrade/reference.py")])
        .arg(source)
        .arg(repository)
        .arg(format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH))
        .output()
        .unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    let actual: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(actual, expected);
}
