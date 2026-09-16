use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

use super::*;
use crate::util::python_json;

#[test]
fn surrogateescape_path_joining_matches_cpython_filesystem_encoding() {
    let mut documents = Vec::new();
    for point in 0xd800..=0xdfff {
        for pattern in
            ["x/./POINT.zip", "../POINT.zip", "/POINT.zip", "//POINT.zip", "///POINT.zip"]
        {
            documents.push(format!("\"{}\"", pattern.replace("POINT", &format!("\\u{point:04x}"))));
        }
    }
    let expected: Vec<Value> = documents
        .iter()
        .map(|document| {
            let python_json::Value::String(ref text) = python_json::parse(document).unwrap() else {
                unreachable!()
            };
            join_text(Path::new("/fixture/cache"), text)
                .map_or(Value::Null, |path| json!(path.as_os_str().as_bytes()))
        })
        .collect();
    let Some(python) = std::env::var_os("HY_TEST_BUNDLE_ORACLE_PYTHON") else {
        return;
    };
    let directory = tempfile::tempdir().unwrap();
    let native_path = directory.path().join(std::ffi::OsStr::from_bytes(b"\xff.zip"));
    let native_probe = match crate::util::python_path::exists(&native_path) {
        Ok(exists) => json!({"exists": exists}),
        Err(error) => json!({"errno": error.raw_os_error()}),
    };
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", include_str!("reference.py")])
        .arg(directory.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&documents).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["probes"], json!([native_probe, {"exists":false}]));
    let actual = report["paths"].as_array().unwrap();
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(actual, &expected, "{}", documents[index]);
    }
    eprintln!(
        "matched {} surrogate filesystem paths and 2 native existence probes",
        documents.len()
    );
}
