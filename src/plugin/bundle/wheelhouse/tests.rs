use std::io::{Cursor, Write};
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::util::python_zip::fixtures::{Member, central_offset, zip};

use super::*;

mod cases;

#[cfg(unix)]
#[test]
fn existing_wheel_symlinks_cannot_redirect_extraction() {
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("wheelhouse");
    let outside = directory.path().join("outside.whl");
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(&outside, b"retain").unwrap();
    std::os::unix::fs::symlink(&outside, destination.join("example.whl")).unwrap();
    let bytes = zip(&[Member::new(b"dependencies/python/target/example.whl", b"replacement")]);
    let mut archive = Archive::new(Cursor::new(bytes)).unwrap();
    assert!(extract(&mut archive, "dependencies/python/target/", &destination).is_err());
    assert_eq!(std::fs::read(outside).unwrap(), b"retain");
}

#[test]
fn wheelhouse_extraction_matches_source_members_and_partial_files() {
    let directory = tempfile::tempdir().unwrap();
    let cases = cases::all();
    let mut inputs = Vec::new();
    let mut expected = Vec::new();
    for (index, case) in cases.iter().enumerate() {
        let archive_path = directory.path().join(format!("{index}.zip"));
        std::fs::write(&archive_path, &case.bytes).unwrap();
        let destination = directory.path().join(format!("out-{index}"));
        let result = Archive::new(Cursor::new(&case.bytes))
            .map_err(Error::from)
            .and_then(|mut archive| extract(&mut archive, &case.prefix, &destination));
        let mut files = std::collections::BTreeMap::new();
        if destination.exists() {
            for entry in std::fs::read_dir(&destination).unwrap() {
                let entry = entry.unwrap();
                let bytes = std::fs::read(entry.path()).unwrap();
                files.insert(
                    entry.file_name().to_str().unwrap().to_owned(),
                    json!({
                        "size": bytes.len(), "sha256": format!("{:x}", Sha256::digest(&bytes)),
                    }),
                );
            }
        }
        expected.push(
            json!({"success": result.is_ok(), "created": destination.exists(), "files": files}),
        );
        inputs.push(json!({"path": archive_path, "prefix": case.prefix}));
    }
    if let Some(python) = std::env::var_os("HY_TEST_BUNDLE_ORACLE_PYTHON") {
        let source =
            std::env::var_os("HY_TEST_HCLI_SOURCE").map(std::path::PathBuf::from).unwrap_or_else(
                || Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli"),
            );
        let mut child = Command::new(python)
            .args(["-I", "-B", "-c", include_str!("reference.py")])
            .arg(source)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(&serde_json::to_vec(&inputs).unwrap()).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(actual.len(), expected.len());
        for (index, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
            assert_eq!(actual, expected, "{}", cases[index].name);
        }
    }
    assert_eq!(cases.len(), 93);
    #[cfg(unix)]
    assert_eq!(
        format!("{:x}", Sha256::digest(serde_json::to_vec(&expected).unwrap())),
        "bcd7f423166b1dcc14b5102c4654bf498951feb0a68915674fc29d7a57fe6bdc",
    );
}
