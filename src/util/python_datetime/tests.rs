use std::io::Write;
use std::process::{Command, Stdio};

use chrono::{Datelike, Timelike};

use super::parse;

#[test]
fn iso_datetime_values_match_cpython() {
    let mut cases = Vec::new();
    for date in [
        "2026-09-15",
        "20260915",
        "2026-W38-2",
        "2026W382",
        "2026-W38",
        "2026W38",
        "0001-01-01",
        "9999-12-31",
        "2024-02-29",
        "2025-02-29",
        "2025-W53-1",
        "2020-W53-7",
        "9999-W52-7",
        "2026-258",
        "２０２６-09-15",
    ] {
        cases.push(date.into());
        for separator in ["T", " ", "😀", "0"] {
            for time in [
                "",
                "12",
                "1234",
                "12:34",
                "123456",
                "12:34:56",
                "12.5",
                "12:34,5",
                "12:34:56.123456789",
                "24:00",
                "12:60",
                "12:34:60",
                "1:23",
                "12:3456",
                "1234:56",
                "12:34.",
            ] {
                for offset in [
                    "",
                    "Z",
                    "+00:00",
                    "-05",
                    "+0530",
                    "+05:30:12.3456789",
                    "+00:00:00.5",
                    "+00:60",
                    "+24:00",
                    "z",
                ] {
                    cases.push(format!("{date}{separator}{time}{offset}"));
                }
            }
        }
    }
    cases.extend(
        [
            "2026-09-15T1234561",
            "2026-09-15T12345612",
            "2026-09-15T123456123456789",
            "2026-09-15T12:34:5612",
            "2026-09-15T12:34:56+05301234",
            "2026-09-15T12:34:56+05:30:1234",
            "2026-09-15T12:34:56-00:00:00.5",
            "2026-09-15T12:34:56+23:59:59.999999",
            "2026-09-15T12:34:56+23:59:60",
            "2026-09-15T12:34:56+00:00:99",
            "2026-09-15\0T12:34:56Z",
            "2026-09-15\u{0}12:34:56Z",
            "2026-09-15T12:34:56Z ",
            "2026-09-15Z12:34:56",
        ]
        .into_iter()
        .map(str::to_owned),
    );
    let actual: Vec<_> = cases
        .iter()
        .map(|value| {
            parse(value).map(|date| {
                [
                    i64::from(date.local.year()),
                    i64::from(date.local.month()),
                    i64::from(date.local.day()),
                    i64::from(date.local.hour()),
                    i64::from(date.local.minute()),
                    i64::from(date.local.second()),
                    i64::from(date.local.nanosecond() / 1000),
                    date.offset_microseconds.unwrap_or(i64::MIN),
                ]
            })
        })
        .collect();
    if let Some(python) = std::env::var_os("HY_TEST_DATETIME_ORACLE_PYTHON") {
        let mut child = Command::new(python).args(["-I", "-B", "-c", r#"
import datetime, json, sys
results = []
for value in json.load(sys.stdin):
    try:
        date = datetime.datetime.fromisoformat(value)
        offset = date.utcoffset()
        microseconds = -(2**63) if offset is None else (offset.days * 86400 + offset.seconds) * 1000000 + offset.microseconds
        results.append([date.year, date.month, date.day, date.hour, date.minute, date.second, date.microsecond, microseconds])
    except ValueError:
        results.append(None)
print(json.dumps(results))
"#]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        child.stdin.take().unwrap().write_all(&serde_json::to_vec(&cases).unwrap()).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let expected: Vec<Option<[i64; 8]>> = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(actual.len(), expected.len());
        for ((value, actual), expected) in cases.iter().zip(&actual).zip(expected) {
            assert_eq!(*actual, expected, "{value:?}");
        }
        eprintln!("matched {} CPython ISO datetime cases", cases.len());
    }
    use sha2::{Digest, Sha256};
    assert_eq!(
        format!("{:x}", Sha256::digest(serde_json::to_vec(&actual).unwrap())),
        "573de527516592a60766c06cad6e7cfb80c110f47030b6cc71c45e85b64c6548"
    );
}
