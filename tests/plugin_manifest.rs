//! Model invariants hold at archive, directory and repository-snapshot boundaries.
#![cfg(unix)]

mod support;

use std::fs;

use serde_json::{Value, json};
use support::*;

#[test]
fn invalid_descriptors_cannot_reach_dependency_installation_or_publication() {
    for source in ["archive", "directory", "snapshot"] {
        for (field, value, diagnostic) in [
            ("authors", json!([]), "authors or maintainers"),
            ("categories", json!(["invalid"]), "unknown plugin category"),
            (
                "settings",
                json!([{"key": "token", "name": "Token", "type": "boolean", "required": false, "secret": true}]),
                "boolean setting",
            ),
            (
                "settings",
                json!([{"key": "token", "name": "Token", "type": "string", "required": false, "choices": ["a"], "validation_pattern": "a"}]),
                "mutually exclusive",
            ),
        ] {
            let sandbox = Sandbox::new();
            let mut descriptor = identity_manifest("1", "https://github.com/example/validation");
            descriptor["plugin"]["pythonDependencies"] = json!(["must-not-resolve"]);
            descriptor["plugin"][field] = value;
            let package = sandbox.path().join("invalid.zip");
            archive_manifest(&package, &descriptor, &[]);
            let directory = sandbox.path().join("source");
            fs::create_dir(&directory).unwrap();
            fs::write(directory.join("ida-plugin.json"), serde_json::to_vec(&descriptor).unwrap())
                .unwrap();
            fs::write(directory.join("plugin.py"), b"# fixture").unwrap();
            let repository = sandbox.path().join("repository.json");
            fs::write(
                &repository,
                serde_json::to_vec(&repository_snapshot(&package, &descriptor)).unwrap(),
            )
            .unwrap();
            let python = sandbox.path().join("python");
            let arguments = sandbox.path().join("pip-arguments");
            fake_python(&python);
            let mut args = vec!["plugin", "--no-python-environment-check"];
            if source == "snapshot" {
                args.extend(["--repo", repository.to_str().unwrap()]);
            }
            args.push("install");
            args.push(match source {
                "archive" => package.to_str().unwrap(),
                "directory" => directory.to_str().unwrap(),
                _ => "example==1",
            });
            let result = sandbox.run_with_env(
                &args,
                &[("HCLI_CURRENT_IDA_PYTHON_EXE", &python), ("HY_TEST_PIP_ARGUMENTS", &arguments)],
            );
            assert!(!result.status.success(), "{source}: {field}");
            // Archive selection skips invalid descriptors; directory and snapshot
            // validation report the rejected field directly.
            let diagnostic = if source == "archive" {
                "ida-plugin.json not found in archive"
            } else {
                diagnostic
            };
            assert!(
                String::from_utf8_lossy(&result.stderr).contains(diagnostic),
                "{source}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(!arguments.exists());
            assert!(!sandbox.path().join("idausr/plugins/example").exists());
        }
    }
}

#[test]
fn repository_reports_serialize_validated_defaults_and_preserve_extra_metadata() {
    let sandbox = Sandbox::new();
    let mut descriptor = identity_manifest("1", "https://github.com/example/validation");
    descriptor["plugin"]["extraField"] = json!({"revision": 42});
    descriptor["plugin"]["settings"] = json!([{"key": "enabled", "name": "Enabled", "type": "boolean", "required": "false", "default": 0}]);
    let repository = sandbox.path().join("repository");
    fs::create_dir(&repository).unwrap();
    archive_manifest(&repository.join("plugin.zip"), &descriptor, &[]);
    let output = sandbox.run(&[
        "plugin",
        "--repo",
        repository.to_str().unwrap(),
        "search",
        "--json",
        "example==1",
    ]);
    assert_success(&output);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let plugin = &report["plugin"];
    assert_eq!(plugin["description"], Value::Null);
    for field in ["categories", "keywords", "maintainers", "pythonDependencies"] {
        assert_eq!(plugin[field], json!([]));
    }
    assert_eq!(plugin["extraField"]["revision"], 42);
    assert_eq!(plugin["settings"][0]["required"], false);
    assert_eq!(plugin["settings"][0]["default"], false);
}
