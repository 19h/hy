//! Regression coverage for the 92df5b1..c10908c upstream changes.

mod support;

use std::fs;
use std::path::Path;

use serde_json::{Value, json};
use support::{Sandbox, archive_manifest, assert_success, identity_manifest};

#[cfg(unix)]
#[path = "upstream_delta/dependencies.rs"]
mod dependencies;

#[cfg(unix)]
#[path = "upstream_delta/bundle.rs"]
mod bundle;

#[cfg(unix)]
#[test]
fn printed_commands_follow_the_invoked_executable_and_quote_its_path() {
    let sandbox = Sandbox::new();
    let alias = sandbox.path().join("hy with spaces");
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_hy"), &alias).unwrap();
    for on_path in [false, true] {
        let output = sandbox
            .command_for(&alias, &["ida", "list"])
            .env_remove("HCLI_BINARY_NAME")
            .env(
                "PATH",
                if on_path {
                    sandbox.path()
                } else {
                    Path::new("/unavailable")
                },
            )
            .output()
            .unwrap();
        assert_success(&output);
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let expected = if on_path {
            "'hy with spaces'".to_owned()
        } else {
            format!("'{}'", alias.display())
        };
        assert!(text.contains(&format!("{expected} ida add --auto")), "{text}");
    }
}

fn descriptor(name: &str, version: &str) -> Value {
    let mut metadata = identity_manifest(version, &format!("https://github.com/example/{name}"));
    metadata["plugin"]["name"] = json!(name);
    metadata
}

fn archive(directory: &Path, name: &str, manifest: &Value) {
    archive_manifest(&directory.join(name), manifest, &[]);
}

#[test]
fn resolves_transitive_dependencies_and_backtracks_before_installation() {
    let sandbox = Sandbox::new();
    let repository = sandbox.path().join("repository");
    fs::create_dir(&repository).unwrap();
    let mut root = descriptor("root", "2");
    root["plugin"]["dependencies"] = json!(["missing"]);
    archive(&repository, "root-2.zip", &root);
    root["plugin"]["version"] = json!("1");
    root["plugin"]["dependencies"] = json!(["child", {"plugin": "optional", "required": false}]);
    archive(&repository, "root-1.zip", &root);
    let mut child = descriptor("child", "1");
    child["plugin"]["dependencies"] = json!(["leaf"]);
    archive(&repository, "child.zip", &child);
    archive(&repository, "leaf.zip", &descriptor("leaf", "1"));
    let output =
        sandbox.run(&["plugin", "--repo", repository.to_str().unwrap(), "install", "root"]);
    assert_success(&output);
    for name in ["root", "child", "leaf"] {
        let metadata: Value = serde_json::from_slice(
            &fs::read(sandbox.path().join(format!("idausr/plugins/{name}/ida-plugin.json")))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(metadata["plugin"]["version"], "1");
    }
    assert!(!sandbox.path().join("idausr/plugins/optional").exists());
    assert!(String::from_utf8_lossy(&output.stderr).contains("optional"));
}

#[test]
fn missing_required_dependency_does_not_publish_any_plugin_or_configuration() {
    let sandbox = Sandbox::new();
    let repository = sandbox.path().join("repository");
    fs::create_dir(&repository).unwrap();
    let mut root = descriptor("root", "1");
    root["plugin"]["dependencies"] = json!(["missing"]);
    archive(&repository, "root.zip", &root);
    let output =
        sandbox.run(&["plugin", "--repo", repository.to_str().unwrap(), "install", "root"]);
    assert!(!output.status.success());
    assert!(!sandbox.path().join("idausr/plugins").exists());
    assert!(!sandbox.path().join("idausr/ida-config.json").exists());
}

#[test]
fn inline_components_share_lifecycle_and_keep_explicit_default_settings() {
    let sandbox = Sandbox::new();
    let mut root = descriptor("suite", "1");
    let mut core = descriptor("core", "1");
    core["plugin"]["settings"] = json!([{
        "key": "enabled",
        "name": "Enabled",
        "type": "boolean",
        "required": false,
        "default": false,
    }]);
    root["plugin"]["components"] = json!([core]);
    let package = sandbox.path().join("suite.zip");
    archive_manifest(&package, &root, &[("package/core/plugin.py", b"# component")]);
    assert_success(&sandbox.run(&[
        "plugin",
        "install",
        package.to_str().unwrap(),
        "--config",
        "core.enabled=false",
    ]));
    assert!(sandbox.path().join("idausr/plugins/suite/core/ida-plugin.json").is_file());
    let config: Value =
        serde_json::from_slice(&fs::read(sandbox.path().join("idausr/ida-config.json")).unwrap())
            .unwrap();
    assert_eq!(config["Plugins"]["core"]["settings"]["enabled"], false);
    assert!(!sandbox.run(&["plugin", "uninstall", "core"]).status.success());
    assert_success(&sandbox.run(&["plugin", "uninstall", "repo/suite"]));
    assert!(!sandbox.path().join("idausr/plugins/suite").exists());
}

#[test]
fn lint_errors_fail_while_recommendations_succeed() {
    let sandbox = Sandbox::new();
    let package = sandbox.path().join("plugin.zip");
    let mut metadata = descriptor("fixture", "1");
    archive_manifest(&package, &metadata, &[]);
    assert_success(&sandbox.run(&["plugin", "lint", package.to_str().unwrap()]));
    metadata["plugin"]["entryPoint"] = json!("absent.py");
    archive_manifest(&package, &metadata, &[]);
    assert_eq!(sandbox.run(&["plugin", "lint", package.to_str().unwrap()]).status.code(), Some(1));
}

#[test]
fn drive_and_stream_archive_members_never_publish() {
    for name in ["D:/outside.py", "D:outside.py", "plugin.py:stream"] {
        let sandbox = Sandbox::new();
        let package = sandbox.path().join("plugin.zip");
        archive_manifest(
            &package,
            &descriptor("fixture", "1"),
            &[(&format!("package/{name}"), b"payload")],
        );
        assert!(!sandbox.run(&["plugin", "install", package.to_str().unwrap()]).status.success());
        assert!(!sandbox.path().join("idausr/plugins/fixture").exists());
    }
}

#[test]
fn noninteractive_license_install_creates_an_explicit_destination() {
    let sandbox = Sandbox::new();
    let license = sandbox.path().join("license.hexlic");
    fs::write(&license, b"fixture license").unwrap();
    let destination = sandbox.path().join("new/installation");
    assert_success(&sandbox.run(&[
        "license",
        "install",
        license.to_str().unwrap(),
        destination.to_str().unwrap(),
    ]));
    assert_eq!(fs::read(destination.join("license.hexlic")).unwrap(), b"fixture license");
    assert!(!sandbox.run(&["license", "install", license.to_str().unwrap()]).status.success());
}

#[cfg(unix)]
#[test]
fn bundle_resolves_each_python_cell_and_runs_uv_without_ida() {
    use std::os::unix::fs::PermissionsExt;
    let sandbox = Sandbox::new();
    let repository = sandbox.path().join("repository");
    fs::create_dir(&repository).unwrap();
    for (version, python) in [("1", ">=3.10"), ("2", ">=3.13")] {
        let mut metadata = descriptor("root", version);
        metadata["plugin"]["requiresPython"] = json!(python);
        metadata["plugin"]["pythonDependencies"] = json!([format!("package{version}==1")]);
        archive(&repository, &format!("root-{version}.zip"), &metadata);
    }
    let bin = sandbox.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let uv = bin.join("uv");
    fs::write(
        &uv,
        r#"#!/bin/sh
printf '%s\n' "$@" >> "$CALLS"
version=$4
while [ $# -gt 0 ]; do
    if [ "$1" = --dest ]; then shift; dest=$1; fi
    shift
done
printf wheel > "$dest/dependency-$version.whl"
"#,
    )
    .unwrap();
    fs::set_permissions(&uv, fs::Permissions::from_mode(0o755)).unwrap();
    let bundle = sandbox.path().join("bundle.zip");
    let calls = sandbox.path().join("calls");
    let output = sandbox
        .command(&[
            "plugin",
            "bundle",
            "create",
            "--repo",
            repository.to_str().unwrap(),
            "--path",
            bundle.to_str().unwrap(),
            "--platform",
            "linux",
            "--python",
            "3.12",
            "--python",
            "3.13",
            "root",
        ])
        .env("PATH", &bin)
        .env("CALLS", &calls)
        .output()
        .unwrap();
    assert_success(&output);
    let mut archive = zip::ZipArchive::new(fs::File::open(bundle).unwrap()).unwrap();
    assert!(archive.by_name("plugins/root-1-linux-x86_64.zip").is_ok());
    assert!(archive.by_name("plugins/root-2-linux-x86_64.zip").is_ok());
    assert!(archive.by_name("dependencies/python/linux-x86_64-cp312/dependency-3.12.whl").is_ok());
    assert!(archive.by_name("dependencies/python/linux-x86_64-cp313/dependency-3.13.whl").is_ok());
    let calls = fs::read_to_string(calls).unwrap();
    assert!(calls.contains("tool\nrun\n--python\n3.12\npip\ndownload"));
    assert!(calls.contains("package1==1"));
    assert!(calls.contains("package2==1"));
}
