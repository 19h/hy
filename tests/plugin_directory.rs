//! Regular source installs follow upstream ZIP packaging and validation rules.
#![cfg(unix)]

#[path = "plugin_directory/selection.rs"]
mod selection;
mod support;

use std::fs;
use std::path::Path;

use serde_json::json;
use support::*;

#[test]
fn source_packaging_preserves_included_files_and_dereferences_file_links() {
    let sandbox = Sandbox::new();
    let source = sandbox.path().join("source");
    fs::create_dir(&source).unwrap();
    fs::write(
        source.join("ida-plugin.json"),
        serde_json::to_vec(&identity_manifest("1", "https://github.com/example/directory"))
            .unwrap(),
    )
    .unwrap();
    fs::write(source.join("plugin.py"), b"# fixture\n").unwrap();
    for directory in [".git", ".hg", ".svn", "__pycache__", ".venv", "venv", ".idea", "empty"] {
        fs::create_dir(source.join(directory)).unwrap();
        if directory != "empty" {
            fs::write(source.join(directory).join("data"), b"fixture").unwrap();
        }
    }
    fs::write(source.join(".DS_Store"), b"excluded").unwrap();
    let external = sandbox.path().join("external.txt");
    fs::write(&external, b"linked file bytes").unwrap();
    std::os::unix::fs::symlink(&external, source.join("file-link")).unwrap();
    std::os::unix::fs::symlink(source.join(".venv"), source.join("directory-link")).unwrap();
    assert_success(&sandbox.run(&["plugin", "install", source.to_str().unwrap()]));
    let target = sandbox.path().join("idausr/plugins/example");
    for included in [".venv", "venv", ".idea"] {
        assert_eq!(fs::read(target.join(included).join("data")).unwrap(), b"fixture");
    }
    for omitted in [".git", ".hg", ".svn", "__pycache__", ".DS_Store", "empty", "directory-link"] {
        assert!(!target.join(omitted).exists(), "{omitted}");
    }
    assert!(!target.join("file-link").is_symlink());
    assert_eq!(fs::read(target.join("file-link")).unwrap(), b"linked file bytes");
    assert!(source.join("file-link").is_symlink());
    assert_eq!(fs::read(external).unwrap(), b"linked file bytes");
}

#[test]
fn regular_directory_install_uses_archive_native_rules_while_editable_uses_directory_rules() {
    for (entry, platforms, regular_valid) in [
        ("native", vec!["linux-x86_64"], true),
        ("native.so", vec!["linux-x86_64"], false),
        ("native", vec!["linux-x86_64", "windows-x86_64"], false),
    ] {
        for editable in [false, true] {
            let sandbox = Sandbox::new();
            let source = sandbox.path().join("source");
            fs::create_dir(&source).unwrap();
            let mut descriptor = identity_manifest("1", "https://github.com/example/directory");
            descriptor["plugin"]["entryPoint"] = json!(entry);
            descriptor["plugin"]["platforms"] = json!(platforms);
            fs::write(source.join("ida-plugin.json"), serde_json::to_vec(&descriptor).unwrap())
                .unwrap();
            fs::write(source.join("native.so"), b"fixture").unwrap();
            let mut args = vec!["plugin", "install"];
            if editable {
                args.push("--editable");
            }
            args.push(source.to_str().unwrap());
            let output = sandbox
                .run_with_env(&args, &[("HCLI_CURRENT_IDA_PLATFORM", Path::new("linux-x86_64"))]);
            let expected = editable || regular_valid;
            assert_eq!(
                output.status.success(),
                expected,
                "{entry} editable={editable}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(sandbox.path().join("idausr/plugins/example").exists(), expected);
        }
    }
}

#[test]
fn dangling_source_file_links_fail_before_publication() {
    let sandbox = Sandbox::new();
    let source = sandbox.path().join("source");
    fs::create_dir(&source).unwrap();
    fs::write(
        source.join("ida-plugin.json"),
        serde_json::to_vec(&identity_manifest("1", "https://github.com/example/directory"))
            .unwrap(),
    )
    .unwrap();
    fs::write(source.join("plugin.py"), b"# fixture\n").unwrap();
    std::os::unix::fs::symlink(source.join("absent"), source.join("dangling")).unwrap();
    let output = sandbox.run(&["plugin", "install", source.to_str().unwrap()]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("No such file or directory"));
    assert!(!sandbox.path().join("idausr/plugins/example").exists());
    assert!(source.join("dangling").is_symlink());
}
