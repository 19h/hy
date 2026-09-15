//! Source path conversion is independent of pip's URL-preservation rule.

use std::fs;
use std::io::Read;
use std::path::Path;

use serde_json::{Value, json};

use super::{Sandbox, assert_success, create, reference};
use crate::support::archive;

#[test]
fn local_paths_expand_home_and_normalize_before_reading() {
    let sandbox = Sandbox::new();
    let package = sandbox.path().join("plugin.zip");
    archive(&package, "1", &[]);
    fs::create_dir(sandbox.path().join("folder")).unwrap();
    fs::create_dir(sandbox.path().join("x:")).unwrap();
    fs::copy(&package, sandbox.path().join("x:/plugin.zip")).unwrap();

    for spec in [
        package.to_str().unwrap(),
        "plugin.zip",
        "./plugin.zip",
        "~/plugin.zip",
        "~//plugin.zip",
        "~/./plugin.zip",
        "~/folder/../plugin.zip",
        "~/x://plugin.zip",
    ] {
        compare_reference(&sandbox, spec, json!({"name": "example", "dependencies": []}));
        let output = sandbox.path().join("bundle.zip");
        assert_success(
            &create(&sandbox, &output, &[Path::new(spec)])
                .args(["--platform", "linux"])
                .current_dir(sandbox.path())
                .output()
                .unwrap(),
        );
        let mut bundle = zip::ZipArchive::new(fs::File::open(&output).unwrap()).unwrap();
        let names: Vec<_> =
            bundle.file_names().filter(|name| name.starts_with("plugins/")).collect();
        assert_eq!(names, ["plugins/example-1.zip"], "{spec}");
        let mut bytes = Vec::new();
        bundle.by_name("plugins/example-1.zip").unwrap().read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, fs::read(&package).unwrap(), "{spec}");
        fs::remove_file(&output).unwrap();
    }
}

#[test]
fn zip_directories_reach_the_file_read_error() {
    let sandbox = Sandbox::new();
    fs::create_dir(sandbox.path().join("directory.zip")).unwrap();
    for spec in ["directory.zip", "~/directory.zip", "~/./directory.zip"] {
        compare_reference(&sandbox, spec, json!({"error_kind": "IsADirectoryError"}));
        let error = create_failure(&sandbox, spec);
        assert!(error.to_lowercase().contains("is a directory"), "{spec}: {error}");
    }
}

#[test]
fn descriptor_errors_retain_the_original_path_spelling() {
    let sandbox = Sandbox::new();
    let package = sandbox.path().join("empty.zip");
    zip::ZipWriter::new(fs::File::create(package).unwrap()).finish().unwrap();
    for spec in ["~/empty.zip", "~/./empty.zip"] {
        let expected = format!("no ida-plugin.json found in {spec}");
        compare_reference(&sandbox, spec, json!({"error": expected}));
        let error = create_failure(&sandbox, spec);
        assert!(error.contains(&expected), "{error}");
    }
}

#[test]
fn unresolved_homes_fail_before_repository_spec_validation() {
    let sandbox = Sandbox::new();
    let name = sandbox.path().file_name().unwrap().to_str().unwrap();
    for suffix in ["/plugin.zip", "://plugin.zip"] {
        let spec = format!("~hy-missing-home-{name}{suffix}");
        let expected = "Could not determine home directory.";
        compare_reference(&sandbox, &spec, json!({"error": expected}));
        let error = create_failure(&sandbox, &spec);
        assert!(error.contains(expected), "{error}");
    }
}

fn compare_reference(sandbox: &Sandbox, spec: &str, expected: Value) {
    reference::compare_with_home(Path::new(spec), Some(sandbox.path()), expected);
}

fn create_failure(sandbox: &Sandbox, spec: &str) -> String {
    let output = sandbox.path().join("bundle.zip");
    let result =
        create(sandbox, &output, &[Path::new(spec)]).current_dir(sandbox.path()).output().unwrap();
    assert!(!result.status.success(), "{spec}: {result:?}");
    assert!(!output.exists(), "{spec}");
    String::from_utf8(result.stderr).unwrap()
}
