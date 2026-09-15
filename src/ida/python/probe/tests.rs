//! Source comparisons for log framing and isolated startup-file selection.

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

use super::*;

fn compare(cases: &[Value]) {
    let Some(python) = std::env::var_os("HY_TEST_VENV_ORACLE_PYTHON") else {
        return;
    };
    let source =
        std::env::var_os("HY_TEST_HCLI_SOURCE").map(std::path::PathBuf::from).unwrap_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli"),
        );
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", include_str!("tests/reference.py")])
        .arg(source)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&json!({"source": SOURCE, "cases": cases})).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{output:?}");
    let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), cases.len());
    for (actual, case) in actual.iter().zip(cases) {
        assert_eq!(*actual, case["expected"], "{case}");
    }
}

#[test]
fn batch_log_selection_and_tail_messages_match_source() {
    let mut cases = Vec::new();
    for separator in [
        "\n", "\r", "\r\n", "\u{b}", "\u{c}", "\u{1c}", "\u{1d}", "\u{1e}", "\u{85}", "\u{2028}",
        "\u{2029}",
    ] {
        for count in [0, 1, 19, 20, 21, 40] {
            let prefix =
                (0..count).map(|index| format!("line {index}{separator}")).collect::<String>();
            for result in [
                "",
                "__hcli__:{\"result\":1}",
                "noise __hcli__:{}",
                "__hcli__:null",
                "__hcli__:invalid\n__hcli__:{}",
            ] {
                for trailing in ["", separator] {
                    let text = format!("{prefix}{result}{trailing}");
                    let expected = match batch::parse(text.as_bytes()) {
                        Ok(value) => json!({"value": value}),
                        Err(Error::Json(_)) => json!({"json_error": true}),
                        Err(error) => json!({"error": error.to_string()}),
                    };
                    cases.push(
                        json!({"mode": "log", "bytes": text.as_bytes(), "expected": expected}),
                    );
                }
            }
        }
    }
    for bytes in [
        b"\xff".as_slice(),
        b"\x00",
        b"__hcli__:{\"path\":\"\xff\"}",
        b"\xef\xbb\xbf__hcli__:{}",
        b"\xff\n__hcli__:{}",
        b"\x85__hcli__:{}",
        b"\r\n\r\n",
        b"__hcli__:{}\n__hcli__:invalid",
    ] {
        let expected = match batch::parse(bytes) {
            Ok(value) => json!({"value": value}),
            Err(Error::Json(_)) => json!({"json_error": true}),
            Err(error) => json!({"error": error.to_string()}),
        };
        cases.push(json!({"mode": "log", "bytes": bytes, "expected": expected}));
    }
    assert_eq!(cases.len(), 668);
    {
        use sha2::{Digest, Sha256};
        let expected: Vec<_> = cases.iter().map(|case| &case["expected"]).collect();
        assert_eq!(
            format!("{:x}", Sha256::digest(serde_json::to_vec(&expected).unwrap())),
            "dfe56f10f6f790598bcd210f6ff72a351e86481e46f46b0443d768b25e28a406",
        );
    }
    compare(&cases);
}

#[test]
fn isolated_probe_copies_only_selected_startup_and_license_files() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source");
    let target = directory.path().join("target");
    let included = [
        "cfg/idapython.cfg",
        "ida.reg",
        "idapythonrc.py",
        "license.hexlic",
        ".hexlic",
        ".hidden.hexlic",
    ];
    let excluded = [
        "plugins/plugin.py",
        "cfg/other.cfg",
        "other.reg",
        "nested/license.hexlic",
        "upper.HEXLIC",
    ];
    for relative in included.into_iter().chain(excluded) {
        let path = source.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, relative).unwrap();
    }
    fs::create_dir(source.join("directory.hexlic")).unwrap();
    files::prepare(&source, &target).unwrap();
    let mut expected = Vec::new();
    for relative in included.into_iter().chain(excluded) {
        let copied = included.contains(&relative) || cfg!(windows) && relative == "upper.HEXLIC";
        assert_eq!(target.join(relative).exists(), copied, "{relative}");
        if copied {
            assert_eq!(fs::read(target.join(relative)).unwrap(), relative.as_bytes());
            assert_eq!(
                fs::metadata(source.join(relative)).unwrap().modified().unwrap(),
                fs::metadata(target.join(relative)).unwrap().modified().unwrap()
            );
            expected.push(relative);
        }
    }
    assert!(!target.join("directory.hexlic").exists());
    expected.sort();
    compare(&[json!({"mode": "files", "source_dir": source, "expected": expected})]);
}
