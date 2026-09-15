use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{ValidationError, parse};

fn outcome(input: &str) -> Value {
    match parse(&serde_json::from_str(input).unwrap()) {
        Ok(value) => json!({"value": value}),
        Err(error) => {
            let kind = match error {
                ValidationError::Type => "datetime_type",
                ValidationError::Parsing {
                    from_date: true,
                    ..
                } => "datetime_from_date_parsing",
                ValidationError::Parsing {
                    from_date: false,
                    ..
                } => "datetime_parsing",
            };
            json!({"error": {"type": kind, "message": error.to_string()}})
        }
    }
}

fn inputs() -> Vec<String> {
    let mut inputs = Vec::new();
    for date in [
        "2026-09-15",
        "2000-02-29",
        "1900-02-29",
        "0000-01-01",
        "0001-01-01",
        "9999-12-31",
        "2026-13-01",
        "2026-01-00",
        "20260915",
        "2026-W38-2",
    ] {
        inputs.push(json!(date).to_string());
        for separator in ["T", "t", " ", "_", "X", "\n"] {
            for time in
                ["00:00", "12:34:56", "23:59:59", "24:00:00", "12:60:00", "12:34:60", "123456"]
            {
                for fraction in ["", ".0", ".123", ".123456", ".123456789", ",123456789", "."] {
                    for timezone in [
                        "",
                        "Z",
                        "z",
                        "+00:00",
                        "-00:00",
                        "+05:30",
                        "-0330",
                        "−03:30",
                        "+24:00",
                        "+01:60",
                        "+01:00:30",
                    ] {
                        inputs.push(
                            json!(format!("{date}{separator}{time}{fraction}{timezone}"))
                                .to_string(),
                        );
                    }
                }
            }
        }
    }
    for number in [
        "0",
        "-0",
        "1",
        "-1",
        "0.0",
        "-0.0",
        "0.0000005",
        "-0.0000005",
        "1.1234567",
        "-1.1234567",
        "20000000000",
        "20000000001",
        "-20000000000",
        "-20000000001",
        "20000000000.999999",
        "20000000001.000001",
        "253402300799999",
        "253402300800000",
        "-62135596800000",
        "-62135596800001",
        "-62167219200000",
        "-62167219200001",
        "999999999999999999",
        "1000000000000000000",
        "-999999999999999999",
        "-1000000000000000000",
        "9223372036854775807",
        "9223372036854775808",
        "-9223372036854775808",
        "-9223372036854775809",
        "1e3",
        "1e-3",
        "1e30",
        "1e999",
        "-1e999",
        "1.0000000000000000001",
    ] {
        inputs.push(number.to_owned());
        inputs.push(json!(number).to_string());
        inputs.push(json!(format!(" {number} ")).to_string());
    }
    for text in
        ["", "NaN", "Infinity", "-Infinity", "2026-09-15 ", " 2026-09-15", "2026-09-15T12:34:56Z\n"]
    {
        inputs.push(json!(text).to_string());
    }
    inputs.extend(["true", "false", "null", "[]", "{}"].map(str::to_owned));
    inputs
}

#[test]
fn bundle_datetimes_match_locked_pydantic_json_validation() {
    let inputs = inputs();
    let expected: Vec<_> = inputs.iter().map(|input| outcome(input)).collect();
    if let Some(python) = std::env::var_os("HY_TEST_VENV_ORACLE_PYTHON") {
        let source =
            std::env::var_os("HY_TEST_HCLI_SOURCE").map(std::path::PathBuf::from).unwrap_or_else(
                || Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli"),
            );
        let mut child = Command::new(python)
            .args(["-I", "-B", "-c", include_str!("reference.py")])
            .arg(source)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(&serde_json::to_vec(&inputs).unwrap()).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(actual.len(), expected.len());
        for ((actual, expected), input) in actual.iter().zip(&expected).zip(&inputs) {
            assert_eq!(actual, expected, "{input}");
        }
    }
    let digest = format!("{:x}", Sha256::digest(serde_json::to_vec(&expected).unwrap()));
    assert_eq!(inputs.len(), 32_470);
    assert_eq!(digest, "8cedc867bd1b317a5de02e81410b4fc89b599005155cfefd01a5f7a180d6caa8");
}
