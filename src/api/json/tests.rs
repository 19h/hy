use serde_json::{Value, json};

use super::decode;

fn encodings(document: &str) -> Vec<Vec<u8>> {
    let utf16_le: Vec<_> = document.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let utf16_be: Vec<_> = document.encode_utf16().flat_map(u16::to_be_bytes).collect();
    let utf32_le: Vec<_> =
        document.chars().flat_map(|value| u32::from(value).to_le_bytes()).collect();
    let utf32_be: Vec<_> =
        document.chars().flat_map(|value| u32::from(value).to_be_bytes()).collect();
    let mut output = Vec::new();
    for (bom, bytes) in [
        (b"\xef\xbb\xbf".as_slice(), document.as_bytes()),
        (b"\xff\xfe".as_slice(), utf16_le.as_slice()),
        (b"\xfe\xff".as_slice(), utf16_be.as_slice()),
        (b"\xff\xfe\0\0".as_slice(), utf32_le.as_slice()),
        (b"\0\0\xfe\xff".as_slice(), utf32_be.as_slice()),
    ] {
        output.push(bytes.to_vec());
        output.push([bom, bytes].concat());
    }
    output
}

#[test]
fn api_json_encodings_preserve_unicode_and_enforce_integer_limits() {
    let documents = [
        r#"{"value":"café-🦀", "nested":[true,null,1.5]}"#.to_owned(),
        r#""emoji \ud83e\udde0 and escaped \\""#.to_owned(),
        "0".into(),
        "[]".into(),
        "null".into(),
        "false".into(),
        "1".repeat(4300),
        "1".repeat(4301),
        format!("{{\"unused\":{}}}", "1".repeat(4301)),
        format!("\"{}\"", "1".repeat(4301)),
    ];
    let mut inputs: Vec<_> = documents.iter().flat_map(|document| encodings(document)).collect();
    inputs.extend([
        b"\xff".to_vec(),
        b"\xff\xfe{".to_vec(),
        b"\0\0\xfe\xff\0".to_vec(),
        b"{\"x\":\"\xc3\"}".to_vec(),
    ]);
    let expected: Vec<Option<Value>> = inputs.iter().map(|input| decode(input).ok()).collect();
    assert_eq!(inputs.len(), 104);
    assert_eq!(expected[0], Some(json!({"value":"café-🦀", "nested":[true,null,1.5]})));
    assert!(expected[70..90].iter().all(Option::is_none));
    assert!(expected[90].is_some());

    let Some(python) = std::env::var_os("HY_TEST_JSON_ORACLE_PYTHON") else {
        return;
    };
    use std::io::Write;
    use std::process::{Command, Stdio};
    let script = r#"
import json, sys
results = []
for value in json.load(sys.stdin):
    try:
        results.append({'valid': True, 'value': json.loads(bytes(value))})
    except ValueError:
        results.append({'valid': False})
print(json.dumps(results))
"#;
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&inputs).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(actual["valid"], expected.is_some(), "case {index}");
        if let Some(expected) = expected {
            assert_eq!(actual["value"], expected, "case {index}");
        }
    }
}
