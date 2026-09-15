//! Bundle creation and consumption through the public CLI with isolated pip and HTTP fixtures.
#![cfg(unix)]

#[path = "plugin_bundle/repository.rs"]
mod repository;
mod support;
#[path = "plugin_bundle/wheelhouse.rs"]
mod wheelhouse;

use std::fs;
use std::io::Write;
use std::path::Path;

use serde_json::{Value, json};
use support::{
    http::{Response, Server},
    *,
};

fn manifest(path: &Path) -> Value {
    let mut archive = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
    serde_json::from_reader(archive.by_name("plugin-bundle.json").unwrap()).unwrap()
}

fn bundle_members(path: &Path) -> Vec<String> {
    zip::ZipArchive::new(fs::File::open(path).unwrap())
        .unwrap()
        .file_names()
        .map(str::to_owned)
        .collect()
}

#[test]
fn pip_offline_mode_keeps_repository_transport_available() {
    let sandbox = Sandbox::new();
    let package = sandbox.path().join("plugin.zip");
    let mut descriptor = identity_manifest("1", "https://github.com/example/offline");
    descriptor["plugin"]["pythonDependencies"] = json!(["fixture==1"]);
    archive_manifest(&package, &descriptor, &[]);
    let snapshot = repository_snapshot(&package, &descriptor);
    let bytes = fs::read(&package).unwrap();
    let server = Server::start(move |request, base| match request.path.as_str() {
        "/repository.json" => {
            let mut snapshot = snapshot.clone();
            snapshot["plugins"][0]["versions"]["1"][0]["url"] = json!(format!("{base}/plugin.zip"));
            Response::json(snapshot)
        }
        "/plugin.zip" => Response::zip(bytes.clone()),
        _ => Response::missing(),
    });
    let python = sandbox.path().join("python");
    let arguments = sandbox.path().join("pip-arguments");
    fake_python(&python);
    let repository = format!("{}/repository.json", server.url);
    assert_success(&sandbox.run_with_env(
        &[
            "plugin",
            "--repo",
            &repository,
            "--offline",
            "--pip-find-links",
            "/fixture/wheels",
            "--no-python-environment-check",
            "install",
            "example==1",
        ],
        &[("HCLI_CURRENT_IDA_PYTHON_EXE", &python), ("HY_TEST_PIP_ARGUMENTS", &arguments)],
    ));
    let requests = server.requests();
    assert_eq!(
        requests.iter().map(|request| request.path.as_str()).collect::<Vec<_>>(),
        ["/repository.json", "/plugin.zip"]
    );
    assert_eq!(fs::read_to_string(arguments).unwrap().matches("--no-index").count(), 2);
    assert_eq!(installed_manifest(&sandbox)["plugin"]["version"], "1");
}

#[test]
fn offline_mode_requires_dependency_sources_but_skips_repository_free_commands() {
    let sandbox = Sandbox::new();
    let package = sandbox.path().join("plugin.zip");
    archive_manifest(&package, &identity_manifest("1", "https://github.com/example/offline"), &[]);
    let output = sandbox.run(&["plugin", "--offline", "install", package.to_str().unwrap()]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("--offline requires --pip-find-links")
    );
    assert!(!sandbox.path().join("idausr/plugins/example").exists());
    assert_success(&sandbox.run(&["plugin", "--offline", "schema"]));
}

#[test]
fn bundle_creation_inherits_sources_and_selects_canonical_platform_variants() {
    for child_override in [false, true] {
        let sandbox = Sandbox::new();
        let repository = sandbox.path().join("repository");
        fs::create_dir(&repository).unwrap();
        for (platform, dependency) in
            [("windows-x86_64", "windows-dependency"), ("linux-aarch64", "linux-dependency")]
        {
            let mut descriptor =
                identity_manifest("2025.09.24", "https://github.com/example/variants");
            descriptor["plugin"]["platforms"] = json!([platform]);
            descriptor["plugin"]["pythonDependencies"] = json!([dependency]);
            archive_manifest(&repository.join(format!("{platform}.zip")), &descriptor, &[]);
        }
        let python = sandbox.path().join("python");
        fake_python(&python);
        let arguments = sandbox.path().join("pip-arguments");
        let output = sandbox.path().join("bundle.zip");
        let unrelated = output.with_extension("tmp.zip");
        fs::write(&unrelated, b"unrelated existing file").unwrap();
        let mut args = vec![
            "plugin",
            "--pip-index-url",
            "https://index.example/simple",
            "--pip-extra-index-url",
            "https://extra.example/simple",
            "--pip-find-links",
            "/wheels with spaces",
            "--offline",
        ];
        if !child_override {
            args.extend(["--repo", repository.to_str().unwrap()]);
        }
        args.extend([
            "bundle",
            "create",
            "--path",
            output.to_str().unwrap(),
            "--platform",
            "windows",
            "--platform",
            "linux-arm64",
            "--python",
            "3.12",
        ]);
        if child_override {
            args.extend(["--repo", repository.to_str().unwrap()]);
        }
        args.push("EXAMPLE==2025.9.24");
        assert_success(&sandbox.run_with_env(
            &args,
            &[("HCLI_CURRENT_IDA_PYTHON_EXE", &python), ("HY_TEST_PIP_ARGUMENTS", &arguments)],
        ));
        let members = bundle_members(&output);
        for platform in ["windows-x86_64", "linux-aarch64"] {
            assert!(members.contains(&format!("plugins/example-2025.09.24-{platform}.zip")));
            assert!(members.contains(&format!(
                "dependencies/python/{platform}-cp312/fixture-1-py3-none-any.whl"
            )));
        }
        let calls = fs::read_to_string(&arguments).unwrap();
        for argument in [
            "windows-dependency",
            "linux-dependency",
            "--no-index",
            "--only-binary=:all:",
            "https://index.example/simple",
            "https://extra.example/simple",
            "/wheels with spaces",
        ] {
            assert_eq!(calls.lines().filter(|line| *line == argument).count(), 2, "{argument}");
        }
        let info = sandbox.run(&["plugin", "bundle", "info", output.to_str().unwrap()]);
        assert_success(&info);
        assert!(String::from_utf8_lossy(&info.stdout).contains("plugins: 1"));
        assert_eq!(fs::read(unrelated).unwrap(), b"unrelated existing file");
        let installed = sandbox
            .command(&[
                "plugin",
                "--repo",
                output.to_str().unwrap(),
                "--offline",
                "--no-python-environment-check",
                "install",
                "example==2025.9.24",
            ])
            .env("HCLI_CURRENT_IDA_PLATFORM", "windows-x86_64")
            .env("HCLI_CURRENT_IDA_PYTHON_EXE", &python)
            .env("HY_TEST_PIP_ARGUMENTS", &arguments)
            .env("HY_TEST_EXPECT_WHEEL", "fixture-1-py3-none-any.whl")
            .output()
            .unwrap();
        assert_success(&installed);
        assert_eq!(installed_manifest(&sandbox)["plugin"]["platforms"], json!(["windows-x86_64"]));
    }
}

#[test]
fn bundle_current_targets_follow_the_selected_interpreter_and_ida_platform() {
    let sandbox = Sandbox::new();
    let package = sandbox.path().join("plugin.zip");
    archive_manifest(&package, &identity_manifest("1", "https://github.com/example/current"), &[]);
    let python = sandbox.path().join("python");
    fake_python(&python);
    let output = sandbox.path().join("bundle.zip");
    let result = sandbox
        .command(&[
            "plugin",
            "bundle",
            "create",
            "--path",
            output.to_str().unwrap(),
            "--platform",
            "current",
            "--python",
            "current",
            package.to_str().unwrap(),
        ])
        .env("HCLI_CURRENT_IDA_PYTHON_EXE", &python)
        .env("HCLI_CURRENT_IDA_PLATFORM", "windows-aarch64")
        .output()
        .unwrap();
    assert_success(&result);
    assert_eq!(manifest(&output)["targetPlatformTags"][0]["id"], "windows-aarch64-cp312");
    let invalid = sandbox.run(&["plugin", "bundle", "info", package.to_str().unwrap()]);
    assert!(!invalid.status.success());
}

#[test]
fn bundle_download_failures_preserve_the_existing_output() {
    for sdist in [false, true] {
        let sandbox = Sandbox::new();
        let package = sandbox.path().join("plugin.zip");
        let mut descriptor = identity_manifest("1", "https://github.com/example/failure");
        descriptor["plugin"]["pythonDependencies"] = json!(["fixture"]);
        archive_manifest(&package, &descriptor, &[]);
        let python = sandbox.path().join("python");
        fake_python(&python);
        let arguments = sandbox.path().join("pip-arguments");
        let output = sandbox.path().join("bundle.zip");
        fs::write(&output, b"original").unwrap();
        if !sdist {
            fs::write(arguments.with_extension("fail-installation"), b"").unwrap();
        }
        let mut command = sandbox.command(&[
            "plugin",
            "bundle",
            "create",
            "--path",
            output.to_str().unwrap(),
            "--target",
            "linux-x86_64-cp312",
            package.to_str().unwrap(),
        ]);
        command
            .env("HCLI_CURRENT_IDA_PYTHON_EXE", &python)
            .env("HY_TEST_PIP_ARGUMENTS", &arguments);
        if sdist {
            command.env("HY_TEST_DOWNLOAD_SDIST", "1");
        }
        let result = command.output().unwrap();
        assert!(!result.status.success());
        assert_eq!(fs::read(&output).unwrap(), b"original");
        assert!(!output.with_extension("tmp.zip").exists());
    }
}

fn fixture_bundle(path: &Path, plugin: &Path, duplicate_wheels: bool) {
    let target = json!({"id": "windows-x86_64-cp312", "idaPlatform": "windows-x86_64", "pythonVersion": "3.12", "implementation": "cp", "abis": ["cp312", "abi3", "none"], "pipPlatformTags": ["win_amd64"], "wheelhouse": "dependencies/python/target"});
    let mut archive = zip::ZipWriter::new(fs::File::create(path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    archive.start_file("plugin-bundle.json", options).unwrap();
    let manifest = json!({"version": 1, "kind": "hcli-plugin-bundle", "builtAt": "2026-09-14T00:00:00Z", "createdBy": {"tool": "hcli", "version": "1"}, "targetPlatformTags": [target]});
    archive.write_all(&serde_json::to_vec(&manifest).unwrap()).unwrap();
    archive.start_file("plugins/example.zip", options).unwrap();
    archive.write_all(&fs::read(plugin).unwrap()).unwrap();
    for directory in if duplicate_wheels {
        &['a', 'b'][..]
    } else {
        &['a'][..]
    } {
        archive
            .start_file(
                format!("dependencies/python/target/{directory}/fixture-1-py3-none-any.whl"),
                options,
            )
            .unwrap();
        archive.write_all(b"fixture wheel").unwrap();
    }
    archive.finish().unwrap();
}

#[test]
fn bundle_install_flattens_wheels_and_rejects_colliding_filenames() {
    for (duplicate, offline) in [(false, true), (false, false), (true, true)] {
        let sandbox = Sandbox::new();
        let package = sandbox.path().join("plugin.zip");
        let mut descriptor = identity_manifest("1", "https://github.com/example/wheelhouse");
        descriptor["plugin"]["pythonDependencies"] = json!(["fixture"]);
        archive_manifest(&package, &descriptor, &[]);
        let bundle = sandbox.path().join("bundle.zip");
        fixture_bundle(&bundle, &package, duplicate);
        let python = sandbox.path().join("python");
        fake_python(&python);
        let arguments = sandbox.path().join("pip-arguments");
        let mut args =
            vec!["plugin", "--repo", bundle.to_str().unwrap(), "--no-python-environment-check"];
        if offline {
            args.push("--offline");
        }
        args.extend(["install", "example==1"]);
        let result = sandbox
            .command(&args)
            .env("HCLI_CURRENT_IDA_PLATFORM", "windows-x86_64")
            .env("HCLI_CURRENT_IDA_PYTHON_EXE", &python)
            .env("HY_TEST_PIP_ARGUMENTS", &arguments)
            .env("HY_TEST_EXPECT_WHEEL", "fixture-1-py3-none-any.whl")
            .output()
            .unwrap();
        if duplicate {
            assert!(!result.status.success());
            assert!(
                String::from_utf8_lossy(&result.stderr)
                    .contains("duplicate filename in wheelhouse")
            );
            assert!(!arguments.exists());
        } else {
            assert_success(&result);
            let calls = fs::read_to_string(arguments).unwrap();
            for argument in ["--isolated", "--no-cache-dir"] {
                assert_eq!(calls.lines().filter(|line| *line == argument).count(), 2);
            }
            assert_eq!(
                calls.matches("--no-index").count(),
                if offline {
                    2
                } else {
                    0
                }
            );
        }
    }
}

#[test]
fn custom_sources_override_a_missing_bundle_target() {
    for custom_sources in [false, true] {
        let sandbox = Sandbox::new();
        let package = sandbox.path().join("plugin.zip");
        let mut descriptor = identity_manifest("1", "https://github.com/example/custom");
        descriptor["plugin"]["pythonDependencies"] = json!(["fixture"]);
        archive_manifest(&package, &descriptor, &[]);
        let bundle = sandbox.path().join("bundle.zip");
        fixture_bundle(&bundle, &package, false);
        let python = sandbox.path().join("python");
        fake_python(&python);
        let arguments = sandbox.path().join("pip-arguments");
        let mut args = vec![
            "plugin",
            "--repo",
            bundle.to_str().unwrap(),
            "--offline",
            "--no-python-environment-check",
        ];
        if custom_sources {
            args.extend(["--pip-find-links", "/external/wheels"]);
        }
        args.extend(["install", "example==1"]);
        let output = sandbox
            .command(&args)
            .env("HCLI_CURRENT_IDA_PLATFORM", "linux-x86_64")
            .env("HCLI_CURRENT_IDA_PYTHON_EXE", &python)
            .env("HY_TEST_PIP_ARGUMENTS", &arguments)
            .output()
            .unwrap();
        if custom_sources {
            assert_success(&output);
            let calls = fs::read_to_string(arguments).unwrap();
            assert_eq!(calls.matches("/external/wheels").count(), 2);
            assert!(!calls.contains("--isolated"));
        } else {
            assert!(!output.status.success());
            assert!(String::from_utf8_lossy(&output.stderr).contains("bundle has no wheelhouse"));
            assert!(!arguments.exists());
            assert!(!sandbox.path().join("idausr/plugins/example").exists());
        }
    }
}

#[test]
fn missing_bundle_targets_are_rejected_even_without_python_dependencies() {
    let sandbox = Sandbox::new();
    let package = sandbox.path().join("plugin.zip");
    archive_manifest(&package, &identity_manifest("1", "https://github.com/example/empty"), &[]);
    let bundle = sandbox.path().join("bundle.zip");
    fixture_bundle(&bundle, &package, false);
    let python = sandbox.path().join("python");
    fake_python(&python);
    let output = sandbox
        .command(&["plugin", "--repo", bundle.to_str().unwrap(), "install", "example==1"])
        .env("HCLI_CURRENT_IDA_PLATFORM", "linux-x86_64")
        .env("HCLI_CURRENT_IDA_PYTHON_EXE", python)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("available targets: windows-x86_64-cp312")
    );
    assert!(!sandbox.path().join("idausr/plugins/example").exists());
}

#[test]
fn bundle_creation_verifies_repository_hashes_before_packaging() {
    for mismatch in ["hash", "identity"] {
        let sandbox = Sandbox::new();
        let package = sandbox.path().join("plugin.zip");
        let descriptor = identity_manifest("1", "https://github.com/example/expected");
        let mut embedded = descriptor.clone();
        if mismatch == "identity" {
            embedded["plugin"]["urls"]["repository"] = json!("https://github.com/example/foreign");
        }
        archive_manifest(&package, &embedded, &[]);
        let mut snapshot = repository_snapshot(&package, &descriptor);
        if mismatch == "hash" {
            snapshot["plugins"][0]["versions"]["1"][0]["sha256"] = json!("0".repeat(64));
        }
        let repository = sandbox.path().join("repository.json");
        fs::write(&repository, serde_json::to_vec(&snapshot).unwrap()).unwrap();
        let output = sandbox.path().join("bundle.zip");
        let result = sandbox.run(&[
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
        ]);
        if mismatch == "hash" {
            assert!(!result.status.success());
            let diagnostic = String::from_utf8_lossy(&result.stderr);
            assert!(diagnostic.contains("hash mismatch: expected"), "{diagnostic}");
            assert!(!output.exists());
        } else {
            assert_success(&result);
            assert!(bundle_members(&output).contains(&"plugins/example-1.zip".into()));
        }
    }
}
