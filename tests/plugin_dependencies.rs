//! Dependency resolution must finish before publishing replacement plugin files.
#![cfg(unix)]

mod support;

use std::fs;

use serde_json::json;
use support::*;

#[test]
fn dependency_failures_preserve_the_previous_plugin() {
    for phase in ["resolution", "installation"] {
        let sandbox = Sandbox::new();
        let interpreter = sandbox.path().join("python");
        fake_python(&interpreter);
        let arguments = sandbox.path().join("arguments");
        fs::write(arguments.with_extension(format!("fail-{phase}")), "").unwrap();
        let original = sandbox.path().join("original.zip");
        archive(&original, "1.0", &[("package/sentinel", b"retain")]);
        assert_success(&sandbox.run(&["plugin", "install", original.to_str().unwrap()]));
        let candidate = sandbox.path().join("candidate.zip");
        archive_with_dependencies(&candidate, "2.0", &[], &["dependency==2"]);

        let output = sandbox.run_with_env(
            &[
                "plugin",
                "--no-python-environment-check",
                "install",
                "-U",
                candidate.to_str().unwrap(),
            ],
            &[("HCLI_CURRENT_IDA_PYTHON_EXE", &interpreter), ("HY_TEST_PIP_ARGUMENTS", &arguments)],
        );
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(&format!("fixture {phase} failure")));
        assert_eq!(
            stderr.contains("Cannot install required Python dependencies:"),
            phase == "resolution"
        );
        assert_eq!(installed_manifest(&sandbox)["plugin"]["version"], "1.0");
        assert_eq!(
            fs::read(sandbox.path().join("idausr/plugins/example/sentinel")).unwrap(),
            b"retain"
        );
        let calls = fs::read_to_string(&arguments).unwrap();
        assert_eq!(
            calls.matches("--invocation--").count(),
            if phase == "resolution" {
                1
            } else {
                2
            }
        );
    }
}

#[test]
fn unsafe_selected_member_is_rejected_before_dependency_resolution() {
    let sandbox = Sandbox::new();
    let interpreter = sandbox.path().join("python");
    fake_python(&interpreter);
    let arguments = sandbox.path().join("arguments");
    let package = sandbox.path().join("unsafe.zip");
    archive_with_dependencies(
        &package,
        "1.0",
        &[("package/../escape", b"invalid")],
        &["dependency"],
    );
    let output = sandbox.run_with_env(
        &["plugin", "--no-python-environment-check", "install", package.to_str().unwrap()],
        &[("HCLI_CURRENT_IDA_PYTHON_EXE", &interpreter), ("HY_TEST_PIP_ARGUMENTS", &arguments)],
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unsafe path"));
    assert!(!arguments.exists());
    assert!(!sandbox.path().join("idausr/plugins/example").exists());
}

#[test]
fn dependency_resolution_includes_other_plugins_and_excludes_the_replaced_version() {
    let sandbox = Sandbox::new();
    let interpreter = sandbox.path().join("python");
    fake_python(&interpreter);
    let arguments = sandbox.path().join("arguments");
    let environment = [
        ("HCLI_CURRENT_IDA_PYTHON_EXE", interpreter.as_path()),
        ("HY_TEST_PIP_ARGUMENTS", arguments.as_path()),
    ];
    let host = "https://github.com/example/plugins";
    for (name, requirement) in [("example", "replaced==1"), ("neighbor", "retained==1")] {
        let mut manifest = identity_manifest("1.0", host);
        manifest["plugin"]["name"] = json!(name);
        let script = format!("# /// script\n# dependencies = ['{requirement}']\n# ///\n");
        if name == "neighbor" {
            manifest["plugin"]["entryPoint"] = json!("inline.py");
            manifest["plugin"]["pythonDependencies"] = json!("inline");
        } else {
            manifest["plugin"]["pythonDependencies"] = json!([requirement]);
        }
        let package = sandbox.path().join(format!("{name}.zip"));
        archive_manifest(&package, &manifest, &[("package/inline.py", script.as_bytes())]);
        assert_success(&sandbox.run_with_env(
            &["plugin", "--no-python-environment-check", "install", package.to_str().unwrap()],
            &environment,
        ));
    }
    fs::remove_file(&arguments).unwrap();
    let mut manifest = identity_manifest("2.0", host);
    manifest["plugin"]["pythonDependencies"] = json!(["replacement==2"]);
    let package = sandbox.path().join("upgrade.zip");
    archive_manifest(&package, &manifest, &[]);
    assert_success(&sandbox.run_with_env(
        &["plugin", "--no-python-environment-check", "install", "-U", package.to_str().unwrap()],
        &environment,
    ));
    let calls = fs::read_to_string(&arguments).unwrap();
    assert_eq!(calls.matches("--invocation--").count(), 2);
    assert_eq!(calls.matches("--dry-run").count(), 1);
    assert_eq!(calls.matches("retained==1").count(), 2);
    assert_eq!(calls.matches("replacement==2").count(), 2);
    assert!(!calls.contains("replaced==1"));
    assert_eq!(installed_manifest(&sandbox)["plugin"]["version"], "2.0");
}

#[test]
fn inline_dependencies_resolve_from_archives_directories_and_editable_sources() {
    for source_kind in ["archive", "directory", "editable"] {
        let sandbox = Sandbox::new();
        let interpreter = sandbox.path().join("python");
        fake_python(&interpreter);
        let arguments = sandbox.path().join("arguments");
        let package = sandbox.path().join("inline.zip");
        let mut manifest = identity_manifest("2025.09.24", "https://github.com/example/inline");
        manifest["plugin"]["entryPoint"] = json!("nested/inline.py");
        manifest["plugin"]["pythonDependencies"] = json!("inline");
        let script = b"# /// script\n# dependencies = ['inline-package[extra]>=1', 'another==2']\n# ///\nraise RuntimeError('plugin code must not execute during installation')\n";
        archive_manifest(&package, &manifest, &[("package/nested/inline.py", script)]);
        let source = if source_kind == "archive" {
            package.clone()
        } else {
            let directory = sandbox.path().join("source");
            zip::ZipArchive::new(fs::File::open(&package).unwrap())
                .unwrap()
                .extract(&directory)
                .unwrap();
            directory.join("package")
        };
        let mut args = vec!["plugin", "--no-python-environment-check", "install"];
        if source_kind == "editable" {
            args.push("--editable");
        }
        args.push(source.to_str().unwrap());
        assert_success(&sandbox.run_with_env(
            &args,
            &[("HCLI_CURRENT_IDA_PYTHON_EXE", &interpreter), ("HY_TEST_PIP_ARGUMENTS", &arguments)],
        ));
        let calls = fs::read_to_string(arguments).unwrap();
        assert_eq!(calls.matches("inline-package[extra]>=1").count(), 2);
        assert_eq!(calls.matches("another==2").count(), 2);
        assert_eq!(installed_manifest(&sandbox)["plugin"]["pythonDependencies"], "inline");
        assert_eq!(installed_manifest(&sandbox)["plugin"]["version"], "2025.09.24");
    }
}

#[test]
fn invalid_inline_dependencies_fail_before_pip_or_publication() {
    for (entry, script) in [
        ("inline.py", "# /// script\n# dependencies = 'wrong type'\n# ///\n"),
        ("inline.py", "# /// script\n# dependencies = [\n# ///\n"),
        ("native", "# /// script\n# dependencies = ['package']\n# ///\n"),
    ] {
        let sandbox = Sandbox::new();
        let interpreter = sandbox.path().join("python");
        fake_python(&interpreter);
        let arguments = sandbox.path().join("arguments");
        let package = sandbox.path().join("invalid.zip");
        let mut manifest = identity_manifest("1.0", "https://github.com/example/inline");
        manifest["plugin"]["entryPoint"] = json!(entry);
        manifest["plugin"]["pythonDependencies"] = json!("inline");
        archive_manifest(
            &package,
            &manifest,
            &[
                (format!("package/{entry}").as_str(), script.as_bytes()),
                ("package/native.so", b"fixture"),
                ("package/native.dll", b"fixture"),
                ("package/native.dylib", b"fixture"),
            ],
        );
        let output = sandbox.run_with_env(
            &["plugin", "--no-python-environment-check", "install", package.to_str().unwrap()],
            &[("HCLI_CURRENT_IDA_PYTHON_EXE", &interpreter), ("HY_TEST_PIP_ARGUMENTS", &arguments)],
        );
        assert!(!output.status.success());
        let expected = if entry.ends_with(".py") {
            "invalid PEP 723 metadata"
        } else {
            "inline dependencies require a Python (.py) entry point"
        };
        assert!(String::from_utf8_lossy(&output.stderr).contains(expected));
        assert!(!arguments.exists());
        assert!(!sandbox.path().join("idausr/plugins/example").exists());
    }
}
