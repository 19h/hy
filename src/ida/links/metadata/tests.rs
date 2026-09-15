use std::io::Write;
use std::process::{Command, Stdio};

use super::validate;

fn assert_cases(cases: &[(Vec<u8>, bool)]) {
    for (bytes, expected) in cases {
        assert_eq!(
            validate(bytes).is_ok(),
            *expected,
            "{} bytes, prefix {:?}",
            bytes.len(),
            &bytes[..bytes.len().min(80)]
        );
    }
    compare_python(cases);
}

fn compare_python(cases: &[(Vec<u8>, bool)]) {
    let Some(python) = std::env::var_os("HY_TEST_JSON_ORACLE_PYTHON") else {
        return;
    };
    const ORACLE: &str = r#"
import json, sys
results = []
for data in json.load(sys.stdin):
    try:
        json.loads(bytes(data))
        results.append(True)
    except (ValueError, RecursionError):
        results.append(False)
json.dump(results, sys.stdout)
"#;
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", ORACLE])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let inputs: Vec<_> = cases.iter().map(|(bytes, _)| bytes).collect();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&inputs).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let outcomes: Vec<bool> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(outcomes.len(), cases.len());
    for ((bytes, expected), actual) in cases.iter().zip(outcomes) {
        assert_eq!(
            actual,
            *expected,
            "CPython: {} bytes, prefix {:?}",
            bytes.len(),
            &bytes[..bytes.len().min(80)]
        );
    }
    eprintln!("CPython metadata oracle matched {} cases", cases.len());
}

fn encodings(document: &str) -> Vec<Vec<u8>> {
    let utf16_le: Vec<_> = document.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let utf16_be: Vec<_> = document.encode_utf16().flat_map(u16::to_be_bytes).collect();
    let utf32_le: Vec<_> = document.chars().flat_map(|c| u32::from(c).to_le_bytes()).collect();
    let utf32_be: Vec<_> = document.chars().flat_map(|c| u32::from(c).to_be_bytes()).collect();
    let mut variants = Vec::new();
    for (bom, encoded) in [
        (b"\xef\xbb\xbf".as_slice(), document.as_bytes()),
        (b"\xff\xfe".as_slice(), utf16_le.as_slice()),
        (b"\xfe\xff".as_slice(), utf16_be.as_slice()),
        (b"\xff\xfe\x00\x00".as_slice(), utf32_le.as_slice()),
        (b"\x00\x00\xfe\xff".as_slice(), utf32_be.as_slice()),
    ] {
        variants.push(encoded.to_vec());
        variants.push([bom, encoded].concat());
    }
    variants
}

#[test]
fn supported_encodings_and_json_values_match_python() {
    let mut cases = Vec::new();
    for (document, expected) in [
        ("null", true),
        (" true\r\n", true),
        ("false", true),
        ("0", true),
        ("-0", true),
        ("-1.25e+20", true),
        ("1e10000", true),
        ("NaN", true),
        ("Infinity", true),
        ("-Infinity", true),
        ("[NaN, Infinity, -Infinity]", true),
        ("{}", true),
        ("[]", true),
        (r#"{"x":1,"x":2}"#, true),
        (r#"{"unicode":"é🦀","surrogate":"\ud800"}"#, true),
        (r#""\udfff\ud800\ud800\udfff""#, true),
        (r#""\"\\\/\b\f\n\r\t""#, true),
        ("", false),
        (" \n\t", false),
        ("[1,]", false),
        ("{\"x\":1,}", false),
        ("{1:2}", false),
        ("null null", false),
        ("01", false),
        ("1.", false),
        ("1e", false),
        ("+1", false),
        ("-NaN", false),
        ("nan", false),
        ("+Infinity", false),
        (r#""\u12x4""#, false),
        (r#""\q""#, false),
        ("\"raw\nnewline\"", false),
        ("\u{a0}null", false),
    ] {
        cases.extend(encodings(document).into_iter().map(|bytes| (bytes, expected)));
    }
    assert_cases(&cases);
}

#[test]
fn surrogatepass_does_not_accept_other_encoding_errors() {
    let cases: &[(&[u8], bool)] = &[
        (b"\"\xed\xa0\x80\"", true),
        (b"\"\xed\xbf\xbf\"", true),
        (b"\"a\xed\xa0\x80b\xed\xb0\x80c\"", true),
        (b"\xff\xfe\x22\x00\x00\xd8\x22\x00", true),
        (b"\xfe\xff\x00\x22\xdf\xff\x00\x22", true),
        (b"\xff\xfe\x00\x00\x22\x00\x00\x00\x00\xd8\x00\x00\x22\x00\x00\x00", true),
        (b"\x00\x00\xfe\xff\x00\x00\x00\x22\x00\x00\xdf\xff\x00\x00\x00\x22", true),
        (b"\xed\xa0\x80", false),
        (b"\"\xff\"", false),
        (b"\"\xed\xa0\"", false),
        (b"\"\xed\xa0\x7f\"", false),
        (b"\"\xc0\xaf\"", false),
        (b"\"\xf4\x90\x80\x80\"", false),
        (b"\xff\xfe\x22\x00\x00", false),
        (b"\x00\x00\xfe\xff\x00\x11\x00\x00", false),
        (b"\x00\x00\xfe\xff\x00", false),
        (b"\xef\xbb\xbf\xef\xbb\xbfnull", false),
        (b"\"\x00\"", false),
    ];
    assert_cases(
        &cases.iter().map(|(bytes, expected)| (bytes.to_vec(), *expected)).collect::<Vec<_>>(),
    );
}

#[test]
fn constants_do_not_change_strings_or_join_adjacent_tokens() {
    let mut cases = Vec::new();
    for constant in ["NaN", "Infinity", "-Infinity"] {
        for (document, expected) in [
            (format!("[{constant}]"), true),
            (format!("{{\"x\":{constant}}}"), true),
            (format!("\"{constant}\""), true),
            (format!("{constant}1"), false),
            (format!("1{constant}"), false),
            (format!("{constant}e1"), false),
            (format!("{constant}.{constant}"), false),
            (format!("[{constant} {constant}]"), false),
            (format!("{{{constant}:1}}"), false),
            (format!("[\"\\\"{constant}\\\\\",{constant}]"), true),
        ] {
            cases.push((document.into_bytes(), expected));
        }
    }
    assert_cases(&cases);
}

#[test]
fn short_token_combinations_match_the_pinned_python_digest() {
    use sha2::{Digest, Sha256};

    let fragments =
        ["0", "-", "1", ".", "e", "NaN", "Infinity", "\"", "\\", " ", ",", ":", "[", "]", "{", "}"];
    let mut level = vec![String::new()];
    let mut cases = Vec::new();
    let mut digest = Sha256::new();
    for length in 0..=3 {
        for body in &level {
            for document in [body.clone(), format!("[{body}]"), format!("{{\"k\":{body}}}")] {
                let bytes = document.into_bytes();
                let accepted = validate(&bytes).is_ok();
                digest.update([u8::from(accepted)]);
                cases.push((bytes, accepted));
            }
        }
        if length < 3 {
            level = level
                .iter()
                .flat_map(|body| fragments.iter().map(move |fragment| format!("{body}{fragment}")))
                .collect();
        }
    }
    assert_eq!(cases.len(), 13_107);
    assert_eq!(cases.iter().filter(|(_, accepted)| *accepted).count(), 266);
    assert_eq!(
        format!("{:x}", digest.finalize()),
        "f961a037e6714b38158435ece87c6f6b085b5c0a1870bd0e6bc2725f7e3f0720"
    );
    compare_python(&cases);
}

#[test]
fn integer_limits_and_deep_documents_match_python_defaults() {
    let mut cases = vec![
        ("9".repeat(4300).into_bytes(), true),
        ("9".repeat(4301).into_bytes(), false),
        (format!("-{}", "9".repeat(4300)).into_bytes(), true),
        (format!("-{}", "9".repeat(4301)).into_bytes(), false),
        (format!("{}.0", "9".repeat(4301)).into_bytes(), true),
        (format!("{}e0", "9".repeat(4301)).into_bytes(), true),
        (format!("1e{}", "9".repeat(4301)).into_bytes(), true),
        (format!("\"{}\"", "9".repeat(4301)).into_bytes(), true),
    ];
    for depth in [128, 200, 900] {
        cases.push((format!("{}0{}", "[".repeat(depth), "]".repeat(depth)).into_bytes(), true));
        cases.push((
            format!("{}0{}", "{\"x\":".repeat(depth), "}".repeat(depth)).into_bytes(),
            true,
        ));
    }
    assert_cases(&cases);
}

#[test]
fn syntax_validation_does_not_recurse_on_the_rust_stack() {
    // Python's interpreter recursion boundary is runtime-dependent. This case
    // verifies the native resource behavior separately from the Python oracle.
    let document = format!("{}0{}", "[".repeat(16_384), "]".repeat(16_384));
    validate(document.as_bytes()).unwrap();
    assert!(validate(&document.as_bytes()[..document.len() - 1]).is_err());
}
