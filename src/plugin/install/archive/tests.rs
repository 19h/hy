use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Write};
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::util::python_zip::fixtures::{Member, central_offset, local_offset, zip};

use super::*;

fn inventory(root: &Path) -> Value {
    let mut files = BTreeMap::new();
    let mut directories = BTreeSet::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            let relative = path.strip_prefix(root).unwrap().to_string_lossy().into_owned();
            let relative = if cfg!(windows) {
                relative.replace('\\', "/")
            } else {
                relative
            };
            if path.is_dir() {
                directories.insert(relative);
                pending.push(path);
            } else {
                let bytes = std::fs::read(path).unwrap();
                files.insert(
                    relative,
                    json!({"size": bytes.len(), "sha256": format!("{:x}", Sha256::digest(bytes))}),
                );
            }
        }
    }
    json!({"success": true, "directories": directories, "files": files})
}

#[test]
fn subtree_extraction_matches_source_selection_validation_and_named_reads() {
    let directory = tempfile::tempdir().unwrap();
    let mut cases = Vec::new();
    for prefix in ["", "pkg/", "./pkg/", "pkg//", "/pkg/", "//pkg/", "../pkg/"] {
        for relative in [
            "file",
            "../bad",
            "nested/../bad",
            ".git/ignored",
            ".git",
            "./.git/kept",
            ".",
            "./",
            "nested/",
            "with\\slash",
            "C:literal",
        ] {
            cases.push((
                prefix.to_owned(),
                zip(&[Member::new(format!("{prefix}{relative}").as_bytes(), b"payload")]),
            ));
        }
    }
    for location in ["pkg/damaged", "ignored/damaged", "pkg/.git/damaged"] {
        for fault in ["symlink", "header", "crc", "encrypted"] {
            let mut bytes = zip(&[
                Member::new(b"pkg/first", b"first"),
                Member::new(location.as_bytes(), b"damaged"),
                Member::new(b"pkg/last", b"last"),
            ]);
            let central = central_offset(&bytes, 1);
            let local = local_offset(&bytes, 1);
            match fault {
                "symlink" => bytes[central + 38..central + 42]
                    .copy_from_slice(&0xa000_0000_u32.to_le_bytes()),
                "header" => bytes[local] = 0,
                "crc" => bytes[central + 16] ^= 1,
                "encrypted" => bytes[central + 8] |= 1,
                _ => unreachable!(),
            }
            cases.push(("pkg/".into(), bytes));
        }
    }
    cases.push((
        "pkg/".into(),
        zip(&[Member::new(b"pkg/duplicate", b"first"), Member::new(b"pkg/duplicate", b"last")]),
    ));
    let mut inputs = Vec::new();
    let mut expected = Vec::new();
    for (index, (prefix, bytes)) in cases.iter().enumerate() {
        let path = directory.path().join(format!("{index}.zip"));
        std::fs::write(&path, bytes).unwrap();
        let destination = directory.path().join(format!("out-{index}"));
        let mut archive = Archive::new(Cursor::new(bytes)).unwrap();
        let result = extract(&mut archive, prefix, &destination);
        expected.push(if result.is_ok() {
            inventory(&destination)
        } else {
            json!({"success": false})
        });
        inputs.push(json!({"path": path, "prefix": prefix}));
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
            assert_eq!(actual, expected, "{}", inputs[index]);
        }
    }
    assert_eq!(inputs.len(), 90);
    #[cfg(unix)]
    assert_eq!(
        format!("{:x}", Sha256::digest(serde_json::to_vec(&expected).unwrap())),
        "b6a4efa2a1478f4c8c8b1f480308543e60a56f2ed9158f95c90f55f640aecb85",
    );
}
