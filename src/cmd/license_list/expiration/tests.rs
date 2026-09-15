use chrono::{NaiveDate, TimeDelta};

use super::at;

#[test]
fn relative_expiration_preserves_day_month_year_and_timezone_boundaries() {
    let now = NaiveDate::from_ymd_opt(2026, 9, 15).unwrap().and_hms_opt(12, 0, 0).unwrap();
    for (days, short, long) in [
        (0, "0d", "expires today"),
        (1, "1d", "expires in 1 day"),
        (30, "30d", "expires in 30 days"),
        (31, "1mo", "expires in 1 month"),
        (365, "12mo", "expires in 12 months"),
        (366, "1y", "expires in 1 year"),
    ] {
        let value = format!("{}+00:00", now + TimeDelta::days(days));
        assert_eq!(at(Some(&value), false, now), short);
        assert_eq!(at(Some(&value), true, now), long);
    }
    for value in
        ["20260915T120000Z", "2026-W38-2T13:00+01", "2026-09-15😀12:00:00.1234567+00:00:00.123456"]
    {
        assert_eq!(at(Some(value), false, now), "0d", "{value}");
        assert_eq!(at(Some(value), true, now), "expires today", "{value}");
    }
    for value in ["2026-09-15", "2026-09-15T12:00", "invalid", "2026-09-15T24:00Z"] {
        assert_eq!(at(Some(value), false, now), value);
        assert_eq!(at(Some(value), true, now), format!("expires {value}"));
    }
    assert_eq!(at(None, false, now), "Never");
    assert_eq!(at(Some(""), true, now), "does not expire");
    assert_eq!(at(Some("2026-09-15T11:59:59.999999Z"), true, now), "expired today");
}

#[test]
fn license_expiration_labels_match_the_upstream_clocked_formatter() {
    let now = NaiveDate::from_ymd_opt(2026, 9, 15).unwrap().and_hms_opt(12, 0, 0).unwrap();
    let mut cases = vec![
        None,
        Some(String::new()),
        Some("invalid".into()),
        Some("2026-09-15".into()),
        Some("2026W382T120000Z".into()),
        Some("2026-09-15T12:00:00.5+00:00:00.5".into()),
    ];
    for days in [-731, -366, -365, -31, -30, -1, 0, 1, 30, 31, 365, 366, 731] {
        for seconds in [-1, 0, 1] {
            cases.push(Some(format!(
                "{}Z",
                now + TimeDelta::days(days) + TimeDelta::seconds(seconds)
            )));
        }
    }
    let expected: Vec<_> = cases
        .iter()
        .map(|value| format!("id Unknown [named] {}", at(value.as_deref(), true, now)))
        .collect();
    if let Some(python) = std::env::var_os("HY_TEST_DATETIME_ORACLE_PYTHON") {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut child = Command::new(python).args(["-I", "-B", "-c", r#"
import ast, datetime, hcli, json, sys, types
from pathlib import Path
path = Path(hcli.__file__).parent / 'commands/license/common.py'
function = next(node for node in ast.parse(path.read_text()).body if isinstance(node, ast.FunctionDef) and node.name == 'license_to_string')
class Clock(datetime.datetime):
    @classmethod
    def now(cls, tz=None):
        fixed = datetime.datetime(2026, 9, 15, 12, tzinfo=datetime.timezone.utc)
        return fixed.astimezone(tz) if tz else fixed.replace(tzinfo=None)
scope = {'datetime': Clock, 'timezone': datetime.timezone, 'License': object}
exec(compile(ast.Module(body=[function], type_ignores=[]), '<upstream-license-format>', 'exec'), scope)
results = []
for value in json.load(sys.stdin):
    license = types.SimpleNamespace(end_date=value, addons=[], edition=None, pubhash='id', license_type='named')
    results.append(scope['license_to_string'](license))
print(json.dumps(results))
"#]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        child.stdin.take().unwrap().write_all(&serde_json::to_vec(&cases).unwrap()).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let actual: Vec<String> = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(actual, expected);
        eprintln!("matched {} upstream license expiration labels", cases.len());
    }
}
