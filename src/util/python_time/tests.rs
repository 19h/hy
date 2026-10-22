use std::io::Write;
use std::process::{Command, Stdio};
#[cfg(unix)]
use std::time::Duration;

use super::*;

#[test]
fn clock_float_bits_match_cpython_at_rounding_boundaries() {
    assert_eq!(clock_seconds(1_700_086_400_000_000_120), 1_700_086_400.0);
    assert_eq!(clock_seconds(-1), -1e-9);
    let mut cases = vec![i64::MIN, i64::MAX, 0];
    for seconds in [0, 1, 9_007_199, 1_700_086_400, 2_000_000_000, 9_223_372_036] {
        for fraction in [0, 1, 120, 500_000_000, 854_775_807, 999_999_999] {
            let Some(base) = (seconds * NANOS_PER_SECOND).checked_add(fraction) else {
                continue;
            };
            for delta in -256..=256 {
                if let Some(value) = base.checked_add(delta) {
                    cases.push(value);
                    if let Some(negative) = value.checked_neg() {
                        cases.push(negative);
                    }
                }
            }
        }
    }
    cases.sort_unstable();
    cases.dedup();
    let expected: Vec<_> = cases.iter().map(|value| clock_seconds(*value).to_bits()).collect();
    let Some(python) = std::env::var_os("HY_TEST_BUNDLE_ORACLE_PYTHON") else {
        return;
    };
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", include_str!("reference.py")])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&cases).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "CPython clock oracle failed");
    let actual: Vec<u64> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), expected.len());
    for ((actual, expected), nanoseconds) in actual.iter().zip(&expected).zip(&cases) {
        assert_eq!(actual, expected, "clock {nanoseconds} ns since Unix epoch");
    }
    eprintln!("matched {} CPython clock float conversions", cases.len());
}

#[test]
#[cfg(unix)]
fn clock_normalizes_negative_fractions_before_checked_nanosecond_arithmetic() {
    assert_eq!(clock(UNIX_EPOCH).unwrap().to_bits(), 0.0_f64.to_bits());
    for nanoseconds in [1, 999_999_999, 1_000_000_001, 1_700_000_000_000_000_120_u64] {
        for positive in [false, true] {
            let duration = Duration::from_nanos(nanoseconds);
            let time = if positive {
                UNIX_EPOCH + duration
            } else {
                UNIX_EPOCH - duration
            };
            let signed = if positive {
                nanoseconds as i64
            } else {
                -(nanoseconds as i64)
            };
            assert_eq!(clock(time).unwrap().to_bits(), clock_seconds(signed).to_bits());
        }
    }
    #[cfg(not(windows))]
    {
        let maximum = UNIX_EPOCH + Duration::from_nanos(i64::MAX as u64);
        assert!(clock(maximum).is_ok());
        assert!(clock(maximum + Duration::from_nanos(1)).is_err());
        let minimum_seconds = UNIX_EPOCH - Duration::from_secs(9_223_372_036);
        assert!(clock(minimum_seconds).is_ok());
        // CPython rejects the seconds multiplication before adding the fraction,
        // even though the final signed nanosecond count would fit in PyTime_t.
        assert!(clock(minimum_seconds - Duration::from_nanos(1)).is_err());
    }
}
