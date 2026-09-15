use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::util::json_format::sorted_ascii;

#[test]
fn snapshot_strings_match_every_unicode_scalar_value() {
    let text: String = (0..=0x10ffff).filter_map(char::from_u32).collect();
    assert_eq!(text.chars().count(), 1_112_064);
    verify(&[json!(text)], "98048830ee35cc22d635ffcbd6d1c1f000f1c37a22f7b09d8260940b48513f04");
}

#[test]
fn snapshot_layout_and_numbers_match_the_source_serialization_pipeline() {
    let mut values = vec![Value::Null, json!(false), json!(true), json!("line\n日本語 🧠\t\u{7f}")];
    for number in [
        "0",
        "-0",
        "1",
        "-1",
        "-0.0",
        "1.0",
        "1e-7",
        "1e-4",
        "1e15",
        "1e16",
        "1e20",
        "1e-20",
        "1.2345678901234567",
        "5e-324",
        "1.7976931348623157e308",
        "1e999",
        "-1e999",
        "18446744073709551616",
    ] {
        values.push(serde_json::from_str(number).unwrap());
    }
    let mut cases = Vec::new();
    for value in values {
        cases.push(value.clone());
        cases.push(json!([value, [], {}]));
        cases.push(json!({"z": value, "a": [], "日本語": {}, "🧠": null}));
        cases.push(json!({"b": {"z": [value], "a": {}}, "a": [[], {}, value]}));
    }
    cases.extend([json!([]), json!({})]);
    assert_eq!(cases.len(), 90);
    verify(&cases, "627e679fb9d2851558ffc291fea75073892271b19c56cafcbc47401bf263b2e1");
}

fn verify(values: &[Value], digest: &str) {
    let expected: Vec<_> = values.iter().map(|value| sorted_ascii(value, "    ")).collect();
    if let Some(python) = std::env::var_os("HY_TEST_BUNDLE_ORACLE_PYTHON") {
        let source =
            std::env::var_os("HY_TEST_HCLI_SOURCE").map(std::path::PathBuf::from).unwrap_or_else(
                || Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli"),
            );
        let documents: Vec<_> =
            values.iter().map(|value| serde_json::to_string(value).unwrap()).collect();
        let mut child = Command::new(python)
            .args(["-I", "-B", "-c", include_str!("reference.py")])
            .arg(source)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(&serde_json::to_vec(&documents).unwrap()).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        let actual: Vec<String> = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(actual.len(), expected.len());
        for (index, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
            assert_eq!(actual.len(), expected.len(), "case {index}: output length");
            assert!(actual == expected, "case {index}: snapshot text differs");
        }
    }
    assert_eq!(format!("{:x}", Sha256::digest(serde_json::to_vec(&expected).unwrap())), digest);
}
