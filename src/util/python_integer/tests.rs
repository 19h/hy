use std::io::Write;
use std::process::{Command, Stdio};

use sha2::{Digest, Sha256};

use super::{digits, parse};

#[test]
fn decimal_syntax_preserves_unicode_signs_and_digit_limits() {
    for (text, expected) in [
        ("+١_۲３", Some("123")),
        ("\u{a0}-९८७\u{3000}", Some("-987")),
        ("-000", Some("0")),
        ("1_2_3", Some("123")),
        ("١__٢", None),
        ("_1", None),
        ("1_", None),
        ("²", None),
        ("Ⅲ", None),
        ("−1", None),
        ("\u{1c}1", None),
        ("", None),
        ("+", None),
    ] {
        assert_eq!(parse(text).map(|value| value.to_string()).as_deref(), expected, "{text:?}");
    }
    assert!(parse(&"9".repeat(4300)).is_some());
    assert!(parse(&"9".repeat(4301)).is_none());
    assert!(parse(&"٠".repeat(4300)).is_some());
    assert!(parse(&"٠".repeat(4301)).is_none());
}

#[test]
fn unicode_decimal_table_matches_the_cpython_digest() {
    let values: Vec<_> = (0..0x110000)
        .map(|code| char::from_u32(code).and_then(digits::decimal).unwrap_or(255))
        .collect();
    assert_eq!(values.iter().filter(|digit| **digit != 255).count(), 680);
    let digest = format!("{:x}", Sha256::digest(&values));
    assert_eq!(digest, "bb3993b130cc02b9f2668f0460278cafc470fd87ebc8b5cccba421b579404a12");
    eprintln!("Unicode decimal table SHA-256 {digest}");
    if let Some(python) = std::env::var_os("HY_TEST_HTTPX_ORACLE_PYTHON") {
        let output = Command::new(python)
            .args(["-I", "-B", "-c", "import hashlib,unicodedata; print(hashlib.sha256(bytes(unicodedata.decimal(chr(n),255) for n in range(0x110000))).hexdigest())"])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), digest);
    }
}

fn corpus() -> Vec<String> {
    let mut cases = Vec::new();
    let alphabet = ['0', '1', '+', '-', '_', ' ', 'a', '١'];
    for length in 0..=4 {
        for mut index in 0..alphabet.len().pow(length) {
            let mut text = String::new();
            for _ in 0..length {
                text.push(alphabet[index % alphabet.len()]);
                index /= alphabet.len();
            }
            cases.push(text);
        }
    }
    for code in 0..0x110000 {
        let Some(character) = char::from_u32(code) else {
            continue;
        };
        if digits::decimal(character).is_some()
            || character.is_whitespace()
            || character.is_ascii_control()
        {
            cases.push(format!("{character}1"));
            cases.push(format!("1{character}"));
            cases.push(format!("+{character}_1"));
        }
    }
    for length in [19, 20, 309, 310, 4300, 4301] {
        for digit in ['0', '9', '٠', '９'] {
            let digits = digit.to_string().repeat(length);
            cases.push(digits.clone());
            cases.push(format!("-{digits}"));
        }
    }
    cases
}

#[test]
fn integer_corpus_matches_cpython() {
    let cases = corpus();
    let actual: Vec<_> =
        cases.iter().map(|text| parse(text).map(|number| number.to_string())).collect();
    let digest = format!("{:x}", Sha256::digest(serde_json::to_vec(&actual).unwrap()));
    assert_eq!(digest, "fbebd0727da7c2bededb7ed78830a5d733431da1e106aacc71ba32c716ebf1d7");
    eprintln!("integer corpus: {} cases, SHA-256 {digest}", cases.len());
    let Some(python) = std::env::var_os("HY_TEST_HTTPX_ORACLE_PYTHON") else {
        return;
    };
    let mut child = Command::new(python)
        .args([
            "-I",
            "-B",
            "-c",
            r#"
import json, sys
results = []
for text in json.load(sys.stdin):
    try:
        results.append(str(int(text)))
    except ValueError:
        results.append(None)
json.dump(results, sys.stdout)
"#,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&cases).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let expected: Vec<Option<String>> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), expected.len());
    for ((text, actual), expected) in cases.iter().zip(actual).zip(expected) {
        assert_eq!(actual, expected, "{text:?}");
    }
}
