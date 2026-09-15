use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;

use super::*;

const HOST: &str = "https://github.com/example/original";
const FOREIGN: &str = "https://github.com/foreign/repo";

fn descriptor(name: &str, host: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "IDAMetadataDescriptorVersion": 1,
        "plugin": {"name": name, "version": "1", "entryPoint": "plugin.py",
            "urls": {"repository": host}, "authors": [{"email": "author@example.test"}]},
    }))
    .unwrap()
}

fn zip(members: &[(&str, &[u8])]) -> Vec<u8> {
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in members {
        archive
            .start_file(
                *name,
                SimpleFileOptions::default().last_modified_time(zip::DateTime::default()),
            )
            .unwrap();
        archive.write_all(bytes).unwrap();
    }
    archive.finish().unwrap().into_inner()
}

#[test]
fn archive_indexing_matches_source_validation_host_filtering_and_failures() {
    let valid = descriptor("example", HOST);
    let other_case = descriptor("EXAMPLE", HOST);
    let foreign = descriptor("foreign", FOREIGN);
    let roots = [("ida-plugin.json", valid.as_slice()), ("plugin.py", b"# fixture".as_slice())];
    let scenarios = [
        zip(&[]),
        zip(&[("other.json", b"not metadata")]),
        zip(&roots),
        zip(&[("bad/ida-plugin.json", b"not JSON"), roots[0], roots[1]]),
        zip(&[("bad/ida-plugin.json", b"\xff"), roots[0], roots[1]]),
        zip(&[("missing/ida-plugin.json", &valid), roots[0], roots[1]]),
        zip(&[
            roots[0],
            roots[1],
            ("missing/ida-plugin.json", &valid),
            ("later/ida-plugin.json", &other_case),
        ]),
        zip(&[("foreign/ida-plugin.json", &foreign), roots[0], roots[1]]),
        zip(&[
            ("foreign/ida-plugin.json", &foreign),
            ("foreign/plugin.py", b"# foreign"),
            roots[0],
            roots[1],
        ]),
        zip(&[
            roots[0],
            roots[1],
            ("later/ida-plugin.json", &other_case),
            ("later/plugin.py", b"# duplicate"),
        ]),
        b"not ZIP".to_vec(),
    ];
    let directory = tempfile::tempdir().unwrap();
    let mut cases = Vec::new();
    let mut expected = Vec::new();
    for (index, bytes) in scenarios.iter().enumerate() {
        let path = directory.path().join(format!("{index}.zip"));
        std::fs::write(&path, bytes).unwrap();
        for host in [
            None,
            Some(""),
            Some(HOST),
            Some(FOREIGN),
            Some("invalid"),
            Some("https://GitHub.com/Example/Original/"),
        ] {
            let mut catalogue = ArchiveCatalogue::default();
            let result = add_bytes(&mut catalogue, bytes, "fixture:archive", host)
                .and_then(|()| catalogue.into_plugins());
            expected.push(match result {
                Ok(plugins) => {
                    let snapshot = crate::plugin::index::Snapshot {
                        version: 1,
                        plugins,
                    };
                    let document: Value =
                        serde_json::from_str(&snapshot.to_json().unwrap()).unwrap();
                    json!({"success": true, "snapshot": document})
                }
                Err(_) => json!({"success": false}),
            });
            cases.push(json!({"path": path, "host": host}));
        }
    }
    assert_eq!(cases.len(), 66);
    compare_source(&cases, &expected);
    assert_eq!(
        format!("{:x}", Sha256::digest(serde_json::to_vec(&expected).unwrap())),
        "109eb8759e7fd8e46014c18a42bbafd91ae9346b97db372fd0cb472acd49bc65",
    );
}

fn compare_source(cases: &[Value], expected: &[Value]) {
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
        child.stdin.take().unwrap().write_all(&serde_json::to_vec(&cases).unwrap()).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(actual.len(), expected.len());
        for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
            assert_eq!(actual, expected, "{}", cases[index]);
        }
    }
}

#[test]
fn duplicate_member_names_match_source_catalogues() {
    use crate::util::python_zip::fixtures::{Member, zip};

    let first = descriptor("first", HOST);
    let last = descriptor("last", HOST);
    let invalid = b"invalid JSON".to_vec();
    let mut scenarios = Vec::new();
    for first_bytes in [&first, &last, &invalid] {
        for last_bytes in [&first, &last, &invalid] {
            scenarios.push(zip(&[
                Member::new(b"ida-plugin.json", first_bytes),
                Member::new(b"ida-plugin.json", last_bytes),
                Member::new(b"plugin.py", b"# fixture"),
            ]));
        }
    }
    for alias in [
        Member::new(b"renamed.json", &last).unicode_path(b"ida-plugin.json"),
        Member::new(b"ida-plugin.json\0suffix", &last),
    ] {
        for reversed in [false, true] {
            let mut members = vec![Member::new(b"ida-plugin.json", &first), alias.clone()];
            if reversed {
                members.reverse();
            }
            members.push(Member::new(b"plugin.py", b"# fixture"));
            scenarios.push(zip(&members));
        }
    }
    let directory = tempfile::tempdir().unwrap();
    let mut cases = Vec::new();
    let mut expected = Vec::new();
    for (index, bytes) in scenarios.iter().enumerate() {
        let path = directory.path().join(format!("{index}.zip"));
        std::fs::write(&path, bytes).unwrap();
        let mut catalogue = ArchiveCatalogue::default();
        add_bytes(&mut catalogue, bytes, "fixture:archive", None).unwrap();
        let snapshot = crate::plugin::index::Snapshot {
            version: 1,
            plugins: catalogue.into_plugins().unwrap(),
        };
        let document: Value = serde_json::from_str(&snapshot.to_json().unwrap()).unwrap();
        cases.push(json!({"path": path, "host": null}));
        expected.push(json!({"success": true, "snapshot": document}));
    }
    compare_source(&cases, &expected);
    assert_eq!(cases.len(), 13);
    assert_eq!(
        format!("{:x}", Sha256::digest(serde_json::to_vec(&expected).unwrap())),
        "69a07a29a2f9be41155315da4e34d40692de3cc37b30e4fe363b7762691f508c",
    );
}
