//! Referenced files are checked consistently across CLI entry points.
#![cfg(unix)]

#[path = "plugin_files/paths.rs"]
mod paths;
mod support;

use std::fs;

use serde_json::{Value, json};
use support::*;

#[test]
fn native_archives_are_discoverable_and_installable_for_every_declared_platform() {
    for (platform, extension) in [
        ("windows-x86_64", "dll"),
        ("windows-aarch64", "dll"),
        ("linux-x86_64", "so"),
        ("linux-aarch64", "so"),
        ("macos-x86_64", "dylib"),
        ("macos-aarch64", "dylib"),
    ] {
        let sandbox = Sandbox::new();
        let repository = sandbox.path().join("repository");
        fs::create_dir(&repository).unwrap();
        let mut descriptor = identity_manifest("1", "https://github.com/example/native");
        descriptor["plugin"]["entryPoint"] = json!("native");
        descriptor["plugin"]["platforms"] = json!([platform]);
        let file = format!("package/native.{extension}");
        archive_manifest(
            &repository.join("native.zip"),
            &descriptor,
            &[(&file, b"native fixture")],
        );
        let search = sandbox
            .command(&[
                "plugin",
                "--repo",
                repository.to_str().unwrap(),
                "search",
                "--json",
                "example==1",
            ])
            .env("HCLI_CURRENT_IDA_PLATFORM", platform)
            .output()
            .unwrap();
        assert_success(&search);
        let report: Value = serde_json::from_slice(&search.stdout).unwrap();
        assert_eq!(report["plugin"]["entryPoint"], "native");
        let install = sandbox
            .command(&[
                "plugin",
                "--repo",
                repository.to_str().unwrap(),
                "--no-python-environment-check",
                "install",
                "example==1",
            ])
            .env("HCLI_CURRENT_IDA_PLATFORM", platform)
            .output()
            .unwrap();
        assert_success(&install);
        assert_eq!(
            fs::read(sandbox.path().join(format!("idausr/plugins/example/native.{extension}")))
                .unwrap(),
            b"native fixture"
        );
    }
}

#[test]
fn missing_or_unsafe_referenced_files_fail_before_pip_and_publication() {
    for (field, value, diagnostic) in [
        ("entryPoint", "absent.py", "entry point file not found"),
        ("entryPoint", "../escape.py", "unsafe entry point"),
        ("entryPoint", "plügïn.py", "unsafe entry point"),
        ("logoPath", "absent.svg", "logo file not found"),
        ("logoPath", "../escape.svg", "unsafe logo"),
    ] {
        for source in ["archive", "directory", "editable"] {
            let sandbox = Sandbox::new();
            let mut descriptor = identity_manifest("1", "https://github.com/example/files");
            descriptor["plugin"][field] = json!(value);
            descriptor["plugin"]["pythonDependencies"] = json!(["must-not-resolve"]);
            let package = sandbox.path().join("plugin.zip");
            archive_manifest(&package, &descriptor, &[]);
            let directory = sandbox.path().join("source");
            fs::create_dir(&directory).unwrap();
            fs::write(directory.join("ida-plugin.json"), serde_json::to_vec(&descriptor).unwrap())
                .unwrap();
            fs::write(directory.join("plugin.py"), b"fixture").unwrap();
            let python = sandbox.path().join("python");
            let arguments = sandbox.path().join("pip-arguments");
            fake_python(&python);
            let input = if source == "archive" {
                &package
            } else {
                &directory
            };
            let mut args = vec!["plugin", "--no-python-environment-check", "install"];
            if source == "editable" {
                args.push("--editable");
            }
            args.push(input.to_str().unwrap());
            let output = sandbox.run_with_env(
                &args,
                &[("HCLI_CURRENT_IDA_PYTHON_EXE", &python), ("HY_TEST_PIP_ARGUMENTS", &arguments)],
            );
            assert!(!output.status.success(), "{source}: {field}");
            assert!(
                String::from_utf8_lossy(&output.stderr).contains(diagnostic),
                "{source}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(!arguments.exists());
            assert!(!sandbox.path().join("idausr/plugins/example").exists());
        }
    }
}
