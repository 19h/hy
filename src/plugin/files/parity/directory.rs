//! Materialize fixture files in Rust; the source validator only reads them.

use std::fs;
use std::os::unix::fs::symlink;

use super::*;

const SCRIPT: &[u8] = b"# /// script\n# dependencies = ['fixture']\n# ///\n";

#[test]
fn directory_references_and_inline_reads_match_source_file_types_and_spelling() {
    let temporary = tempfile::tempdir().unwrap();
    fs::write(temporary.path().join("escape.py"), SCRIPT).unwrap();
    let mut directories = Vec::new();
    let mut expected = Vec::new();
    for field in ["entryPoint", "logoPath"] {
        for (reference, filename) in [
            ("entry.py", Some("entry.py")),
            ("entry.py/", Some("entry.py")),
            ("entry.py/.", Some("entry.py")),
            ("./entry.py", Some("entry.py")),
            ("native", Some("native")),
            ("nested//./entry.py", Some("nested/entry.py")),
            ("C:entry.py", Some("C:entry.py")),
            ("with\\slash.py", Some("with\\slash.py")),
            ("..\\entry.py", Some("..\\entry.py")),
            ("é.py", Some("é.py")),
            ("", None),
            (".", None),
            ("\0.py", None),
            ("../escape.py", None),
        ] {
            for kind in
                ["missing", "file", "directory", "file-link", "directory-link", "dangling", "loop"]
            {
                let directory = temporary.path().join(directories.len().to_string());
                fs::create_dir(&directory).unwrap();
                fs::write(directory.join("main.py"), SCRIPT).unwrap();
                fs::write(directory.join("target"), SCRIPT).unwrap();
                fs::create_dir(directory.join("target-directory")).unwrap();
                if let Some(filename) = filename {
                    let path = directory.join(filename);
                    fs::create_dir_all(path.parent().unwrap()).unwrap();
                    match kind {
                        "missing" => {}
                        "file" => fs::write(path, SCRIPT).unwrap(),
                        "directory" => fs::create_dir(path).unwrap(),
                        "file-link" => symlink(directory.join("target"), path).unwrap(),
                        "directory-link" => {
                            symlink(directory.join("target-directory"), path).unwrap()
                        }
                        "dangling" => symlink(directory.join("absent"), path).unwrap(),
                        "loop" => symlink(&path, &path).unwrap(),
                        _ => unreachable!(),
                    }
                }
                let mut manifest = json!({
                    "IDAMetadataDescriptorVersion": 1,
                    "plugin": {"name": "example", "version": "1", "entryPoint": "main.py",
                        "pythonDependencies": "inline", "authors": [{"email": "a@example.test"}],
                        "urls": {"repository": "https://github.com/example/files"}},
                });
                manifest["plugin"][field] = json!(reference);
                fs::write(
                    directory.join("ida-plugin.json"),
                    serde_json::to_vec(&manifest).unwrap(),
                )
                .unwrap();
                let metadata: PluginMetadata =
                    serde_json::from_value(manifest["plugin"].clone()).unwrap();
                expected.push(json!({
                    "valid": validate_directory_files(&metadata, &directory).is_ok(),
                    "dependencies": crate::plugin::dependencies_from_directory(&metadata, &directory).ok(),
                }));
                directories.push(directory);
            }
        }
    }
    let expected = json!(expected);
    compare_source(json!({"directories": directories}), &expected);
    assert_eq!(directories.len(), 196);
    assert_eq!(
        format!("{:x}", Sha256::digest(serde_json::to_vec(&expected).unwrap())),
        "2acc0c7328e1f1b283a0e6bab9b5ac0ca9e24cf91679df1ce74e0ea2b7b98ee1",
    );
}

#[test]
fn permission_errors_are_not_reclassified_as_missing_references() {
    use std::os::unix::fs::PermissionsExt;

    struct Restore(std::path::PathBuf);
    impl Drop for Restore {
        fn drop(&mut self) {
            let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o700));
        }
    }
    let temporary = tempfile::tempdir().unwrap();
    let directory = temporary.path().join("blocked");
    fs::create_dir(&directory).unwrap();
    let path = directory.join("entry.py");
    fs::write(&path, SCRIPT).unwrap();
    let _restore = Restore(directory.clone());
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o000)).unwrap();
    if !fs::metadata(&path).is_err_and(|error| error.kind() == std::io::ErrorKind::PermissionDenied)
    {
        eprintln!("permission probe unavailable: this process can traverse the blocked directory");
        return;
    }
    let error = crate::util::python_path::exists(&path).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    compare_source(json!({"errors": [path]}), &json!(["error"]));
}
