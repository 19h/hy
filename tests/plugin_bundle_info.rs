//! Compare bundle recognition and inspection with the pinned upstream commands.

use std::fs;
use std::io::{Cursor, Write};
use std::path::Path;

use serde_json::{Value, json};
use zip::write::SimpleFileOptions;

#[path = "plugin_bundle_info/corruption.rs"]
mod corruption;
#[path = "plugin_bundle_info/names.rs"]
mod names;
#[path = "plugin_bundle_info/ordering.rs"]
mod ordering;
#[path = "plugin_bundle_info/reference.rs"]
mod reference;
mod support;

use support::{Sandbox, identity_manifest};

fn zip(members: &[(&str, &[u8])]) -> Vec<u8> {
    zip_with_method(members, zip::CompressionMethod::Deflated)
}

fn zip_with_method(members: &[(&str, &[u8])], method: zip::CompressionMethod) -> Vec<u8> {
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in members {
        archive.start_file(*name, SimpleFileOptions::default().compression_method(method)).unwrap();
        archive.write_all(bytes).unwrap();
    }
    archive.finish().unwrap().into_inner()
}

fn descriptor(name: &str, version: &str) -> Vec<u8> {
    let mut manifest = identity_manifest(version, &format!("https://github.com/example/{name}"));
    manifest["plugin"]["name"] = json!(name);
    serde_json::to_vec(&manifest).unwrap()
}

fn bundle(members: &[(&str, &[u8])]) -> Vec<u8> {
    bundle_at(members, json!("2026-09-15T12:00:00+00:00"))
}

fn bundle_at(members: &[(&str, &[u8])], built_at: Value) -> Vec<u8> {
    let manifest = serde_json::to_vec(&json!({
        "version": 1,
        "kind": "hcli-plugin-bundle",
        "builtAt": built_at,
        "createdBy": {"tool": "hcli", "version": "0.24.0"},
        "targetPlatformTags": [],
    }))
    .unwrap();
    let mut entries = vec![("plugin-bundle.json", manifest.as_slice())];
    entries.extend_from_slice(members);
    zip(&entries)
}

fn inspect(sandbox: &Sandbox, path: &Path, recognized: bool) -> Value {
    let output = sandbox.run(&["plugin", "bundle", "info", path.to_str().unwrap()]);
    let result = json!({
        "recognized": recognized,
        "success": output.status.success(),
        "report": String::from_utf8(output.stdout.clone()).unwrap(),
    });
    reference::compare(path, &result, &output.stderr);
    result
}

fn report(path: &Path, plugins: &str) -> String {
    format!(
        "plugin bundle: {}\n  built: 2026-09-15T12:00:00+00:00\n  created by: hcli 0.24.0\n  targets: \n{plugins}",
        path.display()
    )
}

#[test]
fn recognition_requires_a_lowercase_zip_suffix_and_manifest_presence() {
    let sandbox = Sandbox::new();
    for name in ["bundle.zip", "bundle.ZIP", "bundle.Zip", "bundle", ".zip", "bundle.zip."] {
        let path = sandbox.path().join(name);
        fs::write(&path, bundle(&[])).unwrap();
        let recognized = name == "bundle.zip";
        let result = inspect(&sandbox, &path, recognized);
        assert_eq!(result["success"], recognized, "{name}");
        if recognized {
            assert_eq!(result["report"], report(&path, "  plugins: (none)\n"));
        }
    }
    for (name, data, recognized) in [
        ("broken.zip", b"not a ZIP".to_vec(), false),
        ("empty.zip", zip(&[]), false),
        ("invalid.zip", zip(&[("plugin-bundle.json", b"not JSON")]), true),
    ] {
        let path = sandbox.path().join(name);
        fs::write(&path, data).unwrap();
        assert_eq!(inspect(&sandbox, &path, recognized)["success"], false);
    }
    let directory = sandbox.path().join("directory.zip");
    fs::create_dir(&directory).unwrap();
    assert_eq!(inspect(&sandbox, &directory, false)["success"], false);
}

#[test]
fn invalid_file_references_stop_only_the_current_inner_archive() {
    let sandbox = Sandbox::new();
    let first = descriptor("first", "1");
    let invalid = descriptor("invalid", "1");
    let later = descriptor("later", "1");
    let independent = descriptor("independent", "1");
    let independent_zip =
        zip(&[("ida-plugin.json", &independent), ("plugin.py", b"# independent")]);
    for invalid_first in [true, false] {
        let mut members = vec![("invalid/ida-plugin.json", invalid.as_slice())];
        if !invalid_first {
            members.insert(0, ("first/ida-plugin.json", first.as_slice()));
        }
        members.extend([
            ("first/plugin.py", b"# first".as_slice()),
            ("later/ida-plugin.json", later.as_slice()),
            ("later/plugin.py", b"# later".as_slice()),
        ]);
        let package = zip(&members);
        let path = sandbox.path().join("bundle.zip");
        fs::write(
            &path,
            bundle(&[
                ("plugins/ordered.zip", &package),
                ("plugins/independent.zip", &independent_zip),
            ]),
        )
        .unwrap();
        let result = inspect(&sandbox, &path, true);
        assert_eq!(result["success"], true);
        let expected = if invalid_first {
            "  plugins: 1\n    independent: 1\n"
        } else {
            "  plugins: 2\n    first: 1\n    independent: 1\n"
        };
        assert_eq!(result["report"], report(&path, expected));
    }
}

#[test]
fn descriptor_suffixes_and_lexical_version_order_match_source() {
    let sandbox = Sandbox::new();
    let v2 = descriptor("example", "2");
    let v10 = descriptor("example", "10");
    let package = zip(&[
        ("broken/ida-plugin.json", b"not JSON"),
        ("two/otherida-plugin.json", &v2),
        ("two/plugin.py", b"# two"),
        ("ten/ida-plugin.json", &v10),
        ("ten/plugin.py", b"# ten"),
    ]);
    let path = sandbox.path().join("bundle.zip");
    fs::write(
        &path,
        bundle(&[
            ("plugins/package.zip", &package),
            ("ignored/package.zip", b"not ZIP"),
            ("plugins/ignored.ZIP", b"not ZIP"),
        ]),
    )
    .unwrap();
    let result = inspect(&sandbox, &path, true);
    assert_eq!(result["success"], true);
    assert_eq!(result["report"], report(&path, "  plugins: 1\n    example: 10, 2\n"));
}

#[test]
fn broken_inner_zip_fails_after_printing_manifest_details() {
    let sandbox = Sandbox::new();
    let path = sandbox.path().join("bundle.zip");
    fs::write(&path, bundle(&[("plugins/broken.zip", b"not ZIP")])).unwrap();
    let result = inspect(&sandbox, &path, true);
    assert_eq!(result["success"], false);
    assert_eq!(result["report"], report(&path, ""));
}

#[test]
fn manifest_timestamps_are_validated_and_rendered_as_python_datetimes() {
    let sandbox = Sandbox::new();
    let path = sandbox.path().join("bundle.zip");
    for (input, expected) in [
        (json!("2026-09-15T12:00:00Z"), "2026-09-15T12:00:00+00:00"),
        (json!("2026-09-15"), "2026-09-15T00:00:00"),
        (json!("2026-09-15t12:34:56.123456789z"), "2026-09-15T12:34:56.123456+00:00"),
        (json!("2026-09-15_12:34:56,123-0330"), "2026-09-15T12:34:56.123000-03:30"),
        (json!("2026-09-15 12:34+05:30"), "2026-09-15T12:34:00+05:30"),
        (json!("0001-01-01"), "0001-01-01T00:00:00"),
        (json!(0), "1970-01-01T00:00:00+00:00"),
        (json!(-1.25), "1969-12-31T23:59:58.250000+00:00"),
        (json!("-1.25"), "1969-12-31T23:59:58.750000+00:00"),
        (json!("1700000000000"), "2023-11-14T22:13:20+00:00"),
        (json!(1700000000000_i64), "2023-11-14T22:13:20+00:00"),
    ] {
        fs::write(&path, bundle_at(&[], input.clone())).unwrap();
        let result = inspect(&sandbox, &path, true);
        assert_eq!(result["success"], true, "{input}");
        assert!(
            result["report"].as_str().unwrap().contains(&format!("  built: {expected}\n")),
            "{input}: {result}"
        );
    }
    for input in [
        json!(null),
        json!(true),
        json!([]),
        json!({}),
        json!("not a date"),
        json!("2026-02-29"),
        json!("0000-01-01"),
        json!("2026-09-15T24:00:00Z"),
        json!("2026-09-15T12:34:56+24:00"),
        json!(253402300800000_i64),
    ] {
        fs::write(&path, bundle_at(&[], input.clone())).unwrap();
        let result = inspect(&sandbox, &path, true);
        assert_eq!(result["success"], false, "{input}");
        assert_eq!(result["report"], "", "{input}");
    }
}

#[test]
fn manifest_version_uses_pydantic_literal_equality() {
    let sandbox = Sandbox::new();
    let path = sandbox.path().join("bundle.zip");
    for (version, accepted) in [
        (json!(1), true),
        (json!(1.0), true),
        (json!(true), true),
        (json!("1"), false),
        (json!(false), false),
        (json!(0), false),
        (json!(2), false),
        (json!(1.5), false),
        (json!(null), false),
        (json!([]), false),
    ] {
        let manifest = serde_json::to_vec(&json!({
            "version": version,
            "kind": "hcli-plugin-bundle",
            "builtAt": "2026-09-15T12:00:00+00:00",
            "createdBy": {"tool": "hcli", "version": "0.24.0"},
            "targetPlatformTags": [],
        }))
        .unwrap();
        fs::write(&path, zip(&[("plugin-bundle.json", &manifest)])).unwrap();
        let result = inspect(&sandbox, &path, true);
        assert_eq!(result["success"], accepted, "{version}");
        assert_eq!(
            result["report"],
            if accepted {
                report(&path, "  plugins: (none)\n")
            } else {
                String::new()
            }
        );
    }
}
