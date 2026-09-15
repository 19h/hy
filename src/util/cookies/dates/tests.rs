use std::io::Write;
use std::process::{Command, Stdio};

use serde::Serialize;
use sha2::{Digest, Sha256};

use super::parse;

#[derive(Serialize)]
struct Case {
    text: String,
    year: i32,
}

#[test]
fn expiry_formats_offsets_and_calendar_overflow_have_fixed_results() {
    for (text, expected) in [
        ("Thu, 01 Jan 1970 00:00:00 GMT", Ok(Some(0))),
        ("01-Jan-1970", Ok(Some(0))),
        ("01/01/1970 1:00 +0100", Ok(Some(0))),
        ("01 Jan 1970 00:00 -0100", Ok(Some(3600))),
        ("01 Jan 1970 24:59:61 UTC", Ok(Some(90001))),
        ("31 Feb 1970", Ok(Some(61 * 86400))),
        ("01 Jan 1970 00:00 EST", Ok(None)),
        ("01 Jan 1970 00:00 AM", Ok(None)),
        ("01 Jan 1970 25:00 GMT", Ok(None)),
        ("01 Jan 1970 00:60 GMT", Ok(None)),
        ("01 Jan 1970 00:00:62 GMT", Ok(None)),
        ("Thu, 01 Jax 1970 00:00:00 GMT", Err(())),
        ("01 Jax 1970 00:00:00 GMT", Ok(None)),
    ] {
        assert_eq!(parse(text, 2027), expected, "{text}");
    }
}

#[test]
fn short_years_use_the_local_year_and_a_strict_fifty_year_boundary() {
    for (text, current, resolved) in [
        ("01 Jan 77", 2027, 2077),
        ("01 Jan 78", 2027, 1978),
        ("01 Jan 27", 2077, 2027),
        ("01 Jan 26", 2077, 2126),
        ("01 Jan 0030", 2027, 2030),
        ("01 Jan 999", 2027, 2899),
    ] {
        let expected = chrono::NaiveDate::from_ymd_opt(resolved, 1, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp();
        assert_eq!(parse(text, current), Ok(Some(expected)), "{text}, current year {current}");
    }
    assert_eq!(parse("Thu, 01 Jan 0030 00:00:00 GMT", 2027), Ok(None));
}

fn corpus() -> Vec<Case> {
    let mut cases = Vec::new();
    let mut add = |text: String, year| {
        cases.push(Case {
            text,
            year,
        })
    };
    for year in [1970, 1999, 2027, 2077, 2099] {
        for short in 0..=1000 {
            add(format!("01 Jan {short}"), year);
        }
    }
    for day in [0, 1, 28, 29, 30, 31, 32] {
        for month in ["Jan", "feb", "FEB", "Dec", "Foo", "Jax", "0", "1", "12", "13", "0_2"] {
            for year in ["0000", "1969", "1970", "2000", "2038", "9999", "10000"] {
                for prefix in ["", "Wed, ", "wednesday "] {
                    add(format!("{prefix}{day:02} {month} {year} 00:00:00 GMT"), 2027);
                }
            }
        }
    }
    for hour in [0, 1, 23, 24, 25] {
        for minute in [0, 59, 60] {
            for second in [0, 59, 60, 61, 62] {
                for zone in [
                    "", "GMT", "utc", "UT", "Z", "EST", "AM", "+0100", "-0130", "+99", "123",
                    "9999",
                ] {
                    add(format!("09 Jun 2038 {hour:02}:{minute:02}:{second:02} {zone}"), 2027);
                }
            }
        }
    }
    for text in [
        "09-Jun-2038:1:02",
        "09/Jun/2038 01:02 (UTC)",
        "09 Jun 2038 01:02 +01:00",
        "09 Jun 2038 01:02 PM",
        "09 Jun 2038 01:02 (Pacific)",
        "09 Jun 2038 junk",
        "  Monday, 09 Jun 2038 01:02 GMT\n",
        "\u{1c}09 Jun 2038",
        "09\u{a0}Jun 2038",
        "０９ Jun 2038",
        "09 Jun +2038",
        "Wed, 09 Jun 2038 01:02:03 GMT\n",
        "Wed, 09 Jun 2038 01:02:03 GMT\n\n",
    ] {
        add(text.into(), 2027);
    }
    for count in [10, 4300, 4301] {
        add(format!("09 Jun {}", "0".repeat(count)), 2027);
        add(format!("09 Jun {}", "9".repeat(count)), 2027);
    }
    cases
}

#[test]
fn date_corpus_matches_cpython() {
    let cases = corpus();
    let actual: Vec<_> = cases.iter().map(|case| parse(&case.text, case.year)).collect();
    let digest = format!("{:x}", Sha256::digest(serde_json::to_vec(&actual).unwrap()));
    assert_eq!(digest, "0ad4aef99076e4553bff6257dcd3ea5b922e55f5b082113aebcc5ee5913e1fc4");
    eprintln!("cookie date corpus: {} cases, SHA-256 {digest}", cases.len());
    let Some(python) = std::env::var_os("HY_TEST_HTTPX_ORACLE_PYTHON") else {
        return;
    };
    const ORACLE: &str = r#"
import http.cookiejar, json, sys
from unittest.mock import patch
results = []
for case in json.load(sys.stdin):
    with patch('http.cookiejar.time.localtime', return_value=(case['year'],)):
        try:
            value = http.cookiejar.http2time(case['text'])
            results.append({'Ok': None if value is None else int(value)})
        except ValueError:
            results.append({'Err': None})
json.dump(results, sys.stdout)
"#;
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", ORACLE])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&cases).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let expected: Vec<Result<Option<i64>, ()>> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), expected.len());
    for ((case, actual), expected) in cases.iter().zip(actual).zip(expected) {
        assert_eq!(actual, expected, "year {}: {:?}", case.year, case.text);
    }
}
