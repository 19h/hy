use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::json_str;

#[test]
fn containers_use_python_literals_quoting_and_insertion_order() {
    for (document, expected) in [
        ("null", "None"),
        ("true", "True"),
        ("false", "False"),
        ("-0", "0"),
        ("1.0", "1.0"),
        ("-0.0", "-0.0"),
        ("1e20", "1e+20"),
        ("1e-5", "1e-05"),
        ("1e9999", "inf"),
        ("-1e9999", "-inf"),
        ("-2.0156234722508763e14", "-201562347225087.62"),
        (r#""plain\ntext""#, "plain\ntext"),
        (r#"[null,true,false,1.0]"#, "[None, True, False, 1.0]"),
        (
            r#"{"z":1,"a":["it's","both\"'","\\\n"],"z":2}"#,
            "{'z': 2, 'a': [\"it's\", 'both\"\\\'', '\\\\\\n']}",
        ),
        (
            r#"["café-🦀","\u0000\u000b\u001b\u0085\u00a0\u200b"]"#,
            "['café-🦀', '\\x00\\x0b\\x1b\\x85\\xa0\\u200b']",
        ),
    ] {
        let value = serde_json::from_str(document).unwrap();
        assert_eq!(json_str(&value), expected, "{document}");
    }
}

#[test]
fn printable_classification_matches_every_unicode_code_point() {
    let flags: Vec<_> =
        (0..=0x10ffff).map(|code| u8::from(super::printable::contains(code))).collect();
    assert_eq!(flags.iter().filter(|&&value| value != 0).count(), 149625);
    assert_eq!(
        format!("{:x}", Sha256::digest(&flags)),
        "bf0b550942f03d97e5623f688d8398cc2222ecfa58bfdef567d72d4bb994e4d9"
    );
}

#[test]
fn string_escaping_matches_every_unicode_scalar_value() {
    let mut digest = Sha256::new();
    let mut count = 0;
    for character in (0..=0x10ffff).filter_map(char::from_u32) {
        let mut output = String::new();
        super::strings::write_repr(character.encode_utf8(&mut [0; 4]), &mut output);
        digest.update(output.as_bytes());
        digest.update([0]);
        count += 1;
    }
    assert_eq!(count, 1_112_064);
    assert_eq!(
        format!("{:x}", digest.finalize()),
        "6c6a8f1cd7e21042c3e29990d5fccc93c3b5e8c969136ad1e91f664ee57a0ca3"
    );
}

#[test]
fn binary64_notation_and_rounding_match_cpython() {
    let mut inputs = vec!["0.0".into(), "-0.0".into(), "1e9999".into(), "-1e9999".into()];
    for exponent in -324..=309 {
        for mantissa in ["1", "-1", "1.2345678901234567"] {
            inputs.push(format!("{mantissa}e{exponent}"));
        }
    }
    let mut state = 0x123456789abcdef0u64;
    for _ in 0..32768 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let value = f64::from_bits(state);
        if value.is_finite() {
            inputs.push(format!("{value:e}"));
        }
    }
    assert_eq!(inputs.len(), 34658);
    compare(&inputs, "c0e0ee243a6fc3820a4635ecc6af0b8b9907a6e40cdad53897bb32f937c120d1");
}

#[test]
fn nested_values_and_unicode_strings_match_cpython() {
    let mut inputs = Vec::new();
    for code in (0..=0x10ffff).step_by(257) {
        if let Some(character) = char::from_u32(code) {
            for value in [
                format!("'\"\\\t\n\r{character}"),
                format!("{character}'"),
                format!("{character}\""),
            ] {
                inputs.push(json!([value]).to_string());
            }
        }
    }
    for value in
        [json!(null), json!(true), json!(false), json!({"z":[1.0, false, null], "a":"it's"})]
    {
        inputs.push(value.to_string());
        inputs.push(json!({"nested":[value]}).to_string());
    }
    inputs.push("1".repeat(4300));
    assert_eq!(inputs.len(), 12993);
    compare(&inputs, "06802c207bf3d6e5d6bfb72e805e7d0cb06de8bda5675d6b27e4fa5294c440e6");
}

fn compare(inputs: &[String], expected_digest: &str) {
    let actual: Vec<String> =
        inputs.iter().map(|input| json_str(&serde_json::from_str(input).unwrap())).collect();
    let digest = Sha256::digest(actual.join("\0").as_bytes());
    assert_eq!(format!("{digest:x}"), expected_digest);
    let Some(python) = std::env::var_os("HY_TEST_JSON_ORACLE_PYTHON") else {
        return;
    };
    let script = r#"
import json, sys, unicodedata
assert sys.version_info[:3] == (3, 13, 15)
assert unicodedata.unidata_version == '15.1.0'
print(json.dumps([str(json.loads(value)) for value in json.load(sys.stdin)]))
"#;
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(inputs).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let expected: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), expected.len());
    for ((actual, expected), input) in actual.iter().zip(&expected).zip(inputs) {
        assert_eq!(actual, expected.as_str().unwrap(), "{input}");
    }
}
