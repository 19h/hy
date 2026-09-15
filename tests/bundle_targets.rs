//! Verify the bundle's complete target matrix and the pinned packaging.tags oracle.

mod support;

use std::fs;

use serde_json::Value;
use support::*;

#[test]
fn all_targets_expand_and_match_upstream_macos_platform_tags() {
    let sandbox = Sandbox::new();
    let package = sandbox.path().join("plugin.zip");
    archive_manifest(&package, &identity_manifest("1", "https://github.com/example/targets"), &[]);
    let bundle = sandbox.path().join("bundle.zip");
    assert_success(&sandbox.run(&[
        "plugin",
        "bundle",
        "create",
        "--path",
        bundle.to_str().unwrap(),
        "--platform",
        "all",
        "--platform",
        "linux",
        "--python",
        "all",
        "--python",
        "3.12",
        package.to_str().unwrap(),
    ]));
    let mut archive = zip::ZipArchive::new(fs::File::open(bundle).unwrap()).unwrap();
    let manifest: Value =
        serde_json::from_reader(archive.by_name("plugin-bundle.json").unwrap()).unwrap();
    let targets = manifest["targetPlatformTags"].as_array().unwrap();
    assert_eq!(targets.len(), 30);
    let oracle: Value =
        serde_json::from_str(include_str!("fixtures/macos-platform-tags.json")).unwrap();
    for target in targets {
        let platform = target["idaPlatform"].as_str().unwrap();
        if platform.starts_with("macos-") {
            assert_eq!(target["pipPlatformTags"], oracle[platform]);
        }
        assert!(target["wheelhouse"].as_str().unwrap().ends_with(target["id"].as_str().unwrap()));
        assert_eq!(target["implementation"], "cp");
    }
    assert_eq!(archive.file_names().filter(|name| name.starts_with("plugins/")).count(), 1);
}

#[test]
fn malformed_or_conflicting_targets_fail_before_creating_the_output() {
    for options in [
        vec!["--platform", "linux", "--python", "+3.12"],
        vec!["--platform", "linux", "--python", "3.9"],
        vec!["--target", "linux-x86_64-cp3"],
        vec!["--target", "linux-x86_64-cp312", "--platform", "linux"],
    ] {
        let sandbox = Sandbox::new();
        let output = sandbox.path().join("bundle.zip");
        let mut args = vec!["plugin", "bundle", "create", "--path", output.to_str().unwrap()];
        args.extend(options);
        args.push("example==1");
        assert!(!sandbox.run(&args).status.success());
        assert!(!output.exists());
    }
}
