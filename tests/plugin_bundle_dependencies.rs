//! Bundle creation collects literal requirements; installation resolves inline scripts.
#![cfg(unix)]

use std::fs;
use std::io::Read;
use std::path::Path;

use serde_json::{Value, json};

#[path = "plugin_bundle_dependencies/paths.rs"]
mod paths;
#[path = "plugin_bundle_dependencies/reference.rs"]
mod reference;
mod support;

use support::{Sandbox, archive_manifest, assert_success, fake_python, identity_manifest};

fn descriptor(name: &str, dependencies: Value, entry: &str) -> Value {
    let mut descriptor = identity_manifest("1", &format!("https://github.com/example/{name}"));
    descriptor["plugin"]["name"] = json!(name);
    descriptor["plugin"]["pythonDependencies"] = dependencies;
    descriptor["plugin"]["entryPoint"] = json!(entry);
    descriptor
}

fn create(sandbox: &Sandbox, output: &Path, packages: &[&Path]) -> std::process::Command {
    let mut args = vec![
        "plugin",
        "bundle",
        "create",
        "--path",
        output.to_str().unwrap(),
        "--platform",
        "windows",
        "--python",
        "3.12",
    ];
    args.extend(packages.iter().map(|path| path.to_str().unwrap()));
    sandbox.command(&args)
}

#[test]
fn inline_bundle_creation_does_not_inspect_scripts_or_resolve_python() {
    for (entry, script) in [
        (
            "inline.py",
            Some(b"# /// script\n# dependencies = ['inline-only==1']\n# ///\n".as_slice()),
        ),
        ("inline.py", Some(b"# /// script\n# dependencies = [\n# ///\n".as_slice())),
        ("inline.py", None),
        ("native", Some(b"\xff\0not Python".as_slice())),
    ] {
        let sandbox = Sandbox::new();
        let package = sandbox.path().join("plugin.zip");
        let output = sandbox.path().join("bundle.zip");
        let member = format!("package/{entry}");
        let members: Vec<_> = script.map(|script| (member.as_str(), script)).into_iter().collect();
        archive_manifest(&package, &descriptor("example", json!("inline"), entry), &members);
        reference::compare(&package, json!({"name": "example", "dependencies": []}));
        assert_success(&create(&sandbox, &output, &[&package]).output().unwrap());
        let mut archive = zip::ZipArchive::new(fs::File::open(&output).unwrap()).unwrap();
        assert!(!archive.file_names().any(|name| name.ends_with(".whl")));
        let mut embedded = archive.by_name("plugins/example-1.zip").unwrap();
        let mut bytes = Vec::new();
        embedded.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, fs::read(&package).unwrap());
    }
}

#[test]
fn mixed_archives_preserve_explicit_requirement_order_and_duplicates() {
    let sandbox = Sandbox::new();
    let first = sandbox.path().join("first.zip");
    let inline = sandbox.path().join("inline.zip");
    let second = sandbox.path().join("second.zip");
    archive_manifest(
        &first,
        &descriptor("first", json!(["first==1", "shared==2"]), "plugin.py"),
        &[],
    );
    archive_manifest(&inline, &descriptor("inline", json!("inline"), "missing.py"), &[]);
    archive_manifest(
        &second,
        &descriptor("second", json!(["shared==2", "second==3"]), "plugin.py"),
        &[],
    );
    reference::compare(&first, json!({"name": "first", "dependencies": ["first==1", "shared==2"]}));
    reference::compare(&inline, json!({"name": "inline", "dependencies": []}));
    reference::compare(
        &second,
        json!({"name": "second", "dependencies": ["shared==2", "second==3"]}),
    );
    let output = sandbox.path().join("bundle.zip");
    let python = sandbox.path().join("python");
    let calls = sandbox.path().join("pip-calls");
    fake_python(&python);
    assert_success(
        &create(&sandbox, &output, &[&first, &inline, &second])
            .env("HCLI_CURRENT_IDA_PYTHON_EXE", python)
            .env("HY_TEST_PIP_ARGUMENTS", &calls)
            .output()
            .unwrap(),
    );
    let calls = fs::read_to_string(calls).unwrap();
    assert_eq!(calls.matches("--invocation--\n").count(), 1);
    assert!(calls.ends_with("first==1\nshared==2\nshared==2\nsecond==3\n"), "{calls}");
}

#[test]
fn local_bundle_descriptor_discovery_matches_source_suffix_and_count_rules() {
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    let valid = serde_json::to_vec(&descriptor("example", json!([]), "missing.py")).unwrap();
    for entries in [
        vec![("otherida-plugin.json", valid.as_slice())],
        vec![("ida-plugin.json", b"invalid".as_slice()), ("otherida-plugin.json", &valid)],
        vec![("one/ida-plugin.json", valid.as_slice()), ("two/ida-plugin.json", &valid)],
        vec![("unrelated.json", valid.as_slice())],
    ] {
        let sandbox = Sandbox::new();
        let package = sandbox.path().join("plugin.zip");
        let output = sandbox.path().join("bundle.zip");
        let mut writer = zip::ZipWriter::new(fs::File::create(&package).unwrap());
        for (name, bytes) in &entries {
            writer.start_file(*name, SimpleFileOptions::default()).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
        let expected = match entries[0].0 {
            "one/ida-plugin.json" => json!({
                "error": "plugin archive must contain a single plugin, found: example, example",
            }),
            "unrelated.json" => {
                json!({"error": format!("no ida-plugin.json found in {}", package.display())})
            }
            _ => json!({"name": "example", "dependencies": []}),
        };
        reference::compare(&package, expected.clone());
        let result = create(&sandbox, &output, &[&package]).output().unwrap();
        if let Some(error) = expected.get("error").and_then(Value::as_str) {
            assert!(!result.status.success());
            assert!(String::from_utf8_lossy(&result.stderr).contains(error), "{result:?}");
            assert!(!output.exists());
        } else {
            assert_success(&result);
        }
    }
}

#[test]
fn installing_an_inline_only_bundle_still_resolves_script_dependencies() {
    let sandbox = Sandbox::new();
    let package = sandbox.path().join("plugin.zip");
    let output = sandbox.path().join("bundle.zip");
    archive_manifest(
        &package,
        &descriptor("example", json!("inline"), "inline.py"),
        &[("package/inline.py", b"# /// script\n# dependencies = ['inline-only==1']\n# ///\n")],
    );
    assert_success(&create(&sandbox, &output, &[&package]).output().unwrap());
    let python = sandbox.path().join("python");
    let calls = sandbox.path().join("pip-calls");
    fake_python(&python);
    assert_success(
        &sandbox
            .command(&[
                "plugin",
                "--repo",
                output.to_str().unwrap(),
                "--no-python-environment-check",
                "install",
                "example==1",
            ])
            .env("HCLI_CURRENT_IDA_PLATFORM", "windows-x86_64")
            .env("HCLI_CURRENT_IDA_PYTHON_EXE", python)
            .env("HY_TEST_PIP_ARGUMENTS", &calls)
            .output()
            .unwrap(),
    );
    assert_eq!(fs::read_to_string(calls).unwrap().matches("inline-only==1").count(), 2);
}
