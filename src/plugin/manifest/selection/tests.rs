use std::io::{Cursor, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};
#[cfg(unix)]
use sha2::{Digest, Sha256};

use crate::util::python_zip::fixtures::{Member, central_offset, local_offset, zip};

use super::*;

fn descriptor(name: &str, version: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "IDAMetadataDescriptorVersion": 1,
        "plugin": {"name": name, "version": version, "entryPoint": "plugin.py",
            "urls": {"repository": "https://github.com/example/original"},
            "authors": [{"email": "author@example.test"}]},
    }))
    .unwrap()
}

#[test]
fn selection_and_reference_validation_match_source_independently() {
    let directory = tempfile::tempdir().unwrap();
    let first = descriptor("example", "1");
    let second = descriptor("example", "2");
    let other = descriptor("other", "1");
    let mut scenarios = Vec::new();
    for first in [&first[..], &other, b"invalid JSON", b"\xff"] {
        for last in [&second[..], &other, b"invalid JSON", b"\xff"] {
            for duplicate in [false, true] {
                scenarios.push(zip(&[
                    Member::new(b"a/ida-plugin.json", first),
                    Member::new(
                        if duplicate {
                            b"a/ida-plugin.json"
                        } else {
                            b"b/ida-plugin.json"
                        },
                        last,
                    ),
                    Member::new(b"a/plugin.py", b"# fixture"),
                ]));
            }
        }
    }
    for root in ["", "/", "//", "///", "a/", "./a/", "a//./", "/a/", "//a/", "a/../b/", "a\\b/"] {
        for leaf in ["ida-plugin.json", "extraida-plugin.json"] {
            scenarios.push(zip(&[
                Member::new(format!("{root}{leaf}").as_bytes(), &first),
                Member::new(format!("{root}plugin.py").as_bytes(), b"# fixture"),
            ]));
        }
    }
    for selected in [0, 1, 2] {
        for fault in ["header", "crc", "symlink"] {
            let mut bytes = zip(&[
                Member::new(b"a/ida-plugin.json", &first),
                Member::new(b"b/ida-plugin.json", &second),
                Member::new(b"a/plugin.py", b"# fixture"),
            ]);
            let central = central_offset(&bytes, selected);
            let local = local_offset(&bytes, selected);
            match fault {
                "header" => bytes[local] = 0,
                "crc" => bytes[central + 16] ^= 1,
                "symlink" => bytes[central + 38..central + 42]
                    .copy_from_slice(&0xa000_0000_u32.to_le_bytes()),
                _ => unreachable!(),
            }
            scenarios.push(bytes);
        }
    }
    scenarios.push(zip(&[]));
    scenarios.push(b"invalid ZIP".to_vec());
    let mut inputs = Vec::new();
    let mut expected = Vec::new();
    for (index, bytes) in scenarios.iter().enumerate() {
        let path = directory.path().join(format!("{index}.zip"));
        std::fs::write(&path, bytes).unwrap();
        for name in [None, Some("example"), Some("EXAMPLE"), Some("other"), Some("missing")] {
            let result = Archive::new(Cursor::new(bytes))
                .map_err(Error::from)
                .and_then(|mut archive| {
                    let selected = select_archived_plugin(&mut archive, name)?;
                    let valid = crate::plugin::files::ArchiveReferences::read(&archive)
                        .validate(&selected.metadata, &selected.prefix)
                        .is_ok();
                    Ok(json!({"success": true, "prefix": selected.prefix,
                        "metadata": selected.metadata, "references": valid}))
                })
                .unwrap_or_else(|_| json!({"success": false}));
            inputs.push(json!({"path": path, "name": name}));
            expected.push(result);
        }
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
        let mut stdin = child.stdin.take().unwrap();
        let input = serde_json::to_vec(&inputs).unwrap();
        let writer = std::thread::spawn(move || stdin.write_all(&input).unwrap());
        let output = child.wait_with_output().unwrap();
        writer.join().unwrap();
        assert!(output.status.success());
        let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(actual.len(), expected.len());
        for (index, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
            assert_eq!(actual, expected, "{}", inputs[index]);
        }
    }
    assert_eq!(inputs.len(), 325);
    #[cfg(unix)]
    assert_eq!(
        format!("{:x}", Sha256::digest(serde_json::to_vec(&expected).unwrap())),
        "e31b3c282e461be4b3835b5e2feffb20e359f63708dbd4eb8e81aeca66e0afe6",
    );
}
