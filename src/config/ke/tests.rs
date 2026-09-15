use super::{download_limit, integer};

#[test]
fn integer_settings_accept_signs_spaces_and_digit_separators() {
    for (text, expected) in [
        ("  +1_024\n", Some(1024)),
        ("-1", Some(-1)),
        ("0", Some(0)),
        ("", None),
        ("1__2", None),
        ("_1", None),
        ("1_", None),
        ("1.0", None),
        ("--1", None),
    ] {
        assert_eq!(integer(text), expected.map(num_bigint::BigInt::from), "{text:?}");
    }
}

#[test]
fn settings_use_unicode_integers_and_bound_only_the_byte_counter() {
    for (text, expected) in [
        ("١", 1_048_576),
        ("＋１", u64::MAX),
        ("+１_２", 12 * 1_048_576),
        ("\u{1c}१\u{1f}", 1_048_576),
        ("-१", u64::MAX),
        ("०", u64::MAX),
        ("17592186044415", u64::MAX - 1_048_575),
        ("17592186044416", u64::MAX),
        ("184467440737095516160", u64::MAX),
        ("1__2", u64::MAX),
    ] {
        assert_eq!(download_limit(Some(text)), expected, "{text:?}");
    }
    assert_eq!(download_limit(None), u64::MAX);
    assert!(integer(&"9".repeat(4300)).is_some());
    assert!(integer(&"9".repeat(4301)).is_none());
    assert_eq!(integer("-٩٢٢٣٣٧٢٠٣٦٨٥٤٧٧٥٨٠٩").unwrap().to_string(), "-9223372036854775809");
}

#[test]
fn integer_setting_defaults_and_limits_match_the_upstream_environment_reader() {
    let mut cases = Vec::new();
    for value in [
        "",
        "1",
        "0",
        "-1",
        "+1_024",
        "١",
        "+१_२",
        "９",
        "＋１",
        "1__2",
        "1.0",
        "1 2",
        "9223372036854775808",
        "-9223372036854775809",
        "17592186044416",
    ] {
        for whitespace in ["", " ", "\n", "\u{1c}", "\u{1f}"] {
            cases.push(format!("{whitespace}{value}{whitespace}"));
        }
    }
    cases.extend(["9".repeat(4300), "9".repeat(4301), format!("-{}", "9".repeat(4300))]);
    let actual: Vec<_> = cases
        .iter()
        .map(|value| {
            [
                integer(value).unwrap_or_else(|| 3.into()).to_string(),
                download_limit(Some(value)).to_string(),
            ]
        })
        .collect();
    if let Some(python) = std::env::var_os("HY_TEST_HTTPX_ORACLE_PYTHON") {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut child = Command::new(python)
            .args([
                "-I",
                "-B",
                "-c",
                r#"
import json, os, sys
from hcli.env import _env_int
results = []
for raw in json.load(sys.stdin):
    os.environ['HY_FIXTURE_INTEGER'] = raw
    days = _env_int('HY_FIXTURE_INTEGER', 3)
    limit = max(0, _env_int('HY_FIXTURE_INTEGER', 0))
    byte_limit = min(limit * 1024 * 1024, 2**64 - 1) if limit else 2**64 - 1
    results.append([str(days), str(byte_limit)])
print(json.dumps(results))
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
        let expected: Vec<[String; 2]> = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(actual, expected);
        eprintln!("matched {} upstream integer-setting cases", cases.len());
    }
}
