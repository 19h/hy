use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{Value as Json, json};

use super::*;

mod cases;

fn project(value: &Value) -> Json {
    enum Item<'a> {
        Value(&'a Value),
        Key(&'a Text),
    }
    let mut pending = vec![Item::Value(value)];
    let mut result = Vec::new();
    while let Some(item) = pending.pop() {
        let item = match item {
            Item::Key(text) => {
                result.push(json!({"key": text.0}));
                continue;
            }
            Item::Value(value) => value,
        };
        result.push(match item {
            Value::Null => json!({"null": true}),
            Value::Bool(value) => json!({"bool": value}),
            Value::Integer(value) => json!({"int": value.to_string()}),
            Value::Float(value) => json!({"float": format!("{:016x}", value.to_bits())}),
            Value::String(text) => json!({"str": text.0}),
            Value::Array(values) => {
                pending.extend(values.iter().rev().map(Item::Value));
                json!({"array": values.len()})
            }
            Value::Object(object) => {
                for (key, value) in object.0.iter().rev() {
                    pending.push(Item::Value(value));
                    pending.push(Item::Key(key));
                }
                json!({"object": object.0.len()})
            }
        });
    }
    json!({"values": result})
}

#[test]
fn values_encodings_and_syntax_match_cpython_json_loads() {
    let documents = cases::documents();
    assert_eq!(documents.len(), 2012);
    let expected: Vec<_> = documents
        .iter()
        .map(|document| {
            decode(document).map_or_else(|_| json!({"error": true}), |value| project(&value))
        })
        .collect();
    let Some(python) = std::env::var_os("HY_TEST_BUNDLE_ORACLE_PYTHON") else {
        return;
    };
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", include_str!("reference.py")])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&documents).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let report: Json = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["zip_folds"], json!({"z": [90, 122], "i": [73, 105], "p": [80, 112]}));
    let actual = report["results"].as_array().unwrap();
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
        assert_eq!(actual, expected, "case {index}: {} bytes", documents[index].len());
    }
}

#[test]
fn scalar_types_and_private_serde_number_keys_keep_their_identity() {
    let value = decode(
        br#"{
        "x":NaN,"x":Infinity,"negative":-Infinity,
        "large":18446744073709551616,"$serde_json::private::Number":"4"
    }"#,
    )
    .unwrap();
    let Value::Object(object) = &value else {
        panic!("expected object")
    };
    assert!(matches!(object.get("x"), Some(Value::Float(value)) if *value == f64::INFINITY));
    assert!(
        matches!(object.get("negative"), Some(Value::Float(value)) if *value == f64::NEG_INFINITY)
    );
    let Some(Value::Integer(value)) = object.get("large") else {
        panic!("expected an arbitrary-precision integer");
    };
    assert_eq!(value.to_string(), "18446744073709551616");
    let Some(Value::String(value)) = object.get("$serde_json::private::Number") else {
        panic!("expected an ordinary object field");
    };
    assert_eq!(value.to_utf8().unwrap(), "4");
    let Value::Float(value) = decode(b"NaN").unwrap() else {
        panic!("expected float")
    };
    assert!(value.is_nan());
    assert!(decode(b"nan").is_err());
    assert!(decode(b"-NaN").is_err());
}

#[test]
fn surrogate_strings_and_deep_containers_do_not_use_serde_limits() {
    let Value::String(ref text) = decode(br#""\ud800a\udfff.ZIP""#).unwrap() else {
        panic!("expected string")
    };
    assert!(text.has_zip_suffix());
    assert!(text.to_utf8().is_err());
    assert_eq!(text.diagnostic(), "\\ud800a\\udfff.ZIP");
    for depth in [128, 256, 512, 900, 1100, 10000] {
        let document = format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
        assert!(decode(document.as_bytes()).is_ok());
    }
}
