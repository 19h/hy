use chrono::{NaiveDate, TimeDelta};

use super::*;

#[test]
fn credential_dates_share_their_fallback_boundary() {
    assert_eq!(
        credentials("20260915T123456Z", "2026-W38-2"),
        ("2026-09-15 12:34".into(), "2026-09-15 00:00".into())
    );
    assert_eq!(
        credentials("20260915T123456Z", "invalid"),
        ("20260915T123456Z".into(), "invalid".into())
    );
    assert_eq!(credentials("", "2026-09-15T12:34:56Z"), ("N/A".into(), "2026-09-15T12:34".into()));
    assert_eq!(key_created("2026W382"), "Sep 15 2026");
    assert_eq!(key_created("invalid"), "Unknown");
}

#[test]
fn relative_key_times_preserve_strict_thresholds_and_negative_day_remainders() {
    let now = NaiveDate::from_ymd_opt(2026, 9, 15).unwrap().and_hms_opt(12, 0, 0).unwrap();
    for (seconds, expected) in [
        (0, "just now"),
        (60, "just now"),
        (61, "1 minute ago"),
        (3600, "60 minutes ago"),
        (3601, "1 hour ago"),
        (7200, "2 hours ago"),
        (86400, "1 day ago"),
        (172800, "2 days ago"),
        (-1, "23 hours ago"),
    ] {
        let value = format!("{}Z", now - TimeDelta::seconds(seconds));
        assert_eq!(
            relative_at(Some(&value), now, now + TimeDelta::hours(9)),
            expected,
            "{seconds}"
        );
    }
    assert_eq!(
        relative_at(Some("2026-09-15T12:00:00"), now, now + TimeDelta::hours(2)),
        "2 hours ago"
    );
    assert_eq!(relative_at(Some("invalid"), now, now), "unknown");
    assert_eq!(relative_at(None, now, now), "never");

    if let Some(python) = std::env::var_os("HY_TEST_DATETIME_ORACLE_PYTHON") {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut cases = vec![
            None,
            Some(String::new()),
            Some("invalid".into()),
            Some("2026W382T123456Z".into()),
            Some("2026-09-15".into()),
            Some("2026-09-15T12:00:00.5+00:00:00.5".into()),
        ];
        for seconds in
            [-172801, -86400, -3601, -3600, -61, -60, -1, 0, 1, 60, 61, 3600, 3601, 86400, 172800]
        {
            cases.push(Some(format!("{}Z", now - TimeDelta::seconds(seconds))));
        }
        let expected: Vec<_> = cases
            .iter()
            .map(|value| {
                [
                    key_created(value.as_deref().unwrap_or("")),
                    relative_at(value.as_deref(), now, now),
                ]
            })
            .collect();
        let mut child = Command::new(python).args(["-I", "-B", "-c", r#"
import ast, datetime, hcli, json, sys
from pathlib import Path
path = Path(hcli.__file__).parent / 'commands/auth/key/list.py'
functions = [node for node in ast.parse(path.read_text()).body if isinstance(node, ast.FunctionDef) and node.name in ('format_datetime', 'format_relative_time')]
class Clock(datetime.datetime):
    @classmethod
    def now(cls, tz=None):
        fixed = datetime.datetime(2026, 9, 15, 12, tzinfo=datetime.timezone.utc)
        return fixed.astimezone(tz) if tz else fixed.replace(tzinfo=None)
scope = {'datetime': Clock}
exec(compile(ast.Module(body=functions, type_ignores=[]), '<upstream-key-dates>', 'exec'), scope)
print(json.dumps([[scope['format_datetime'](value), scope['format_relative_time'](value)] for value in json.load(sys.stdin)]))
"#]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        child.stdin.take().unwrap().write_all(&serde_json::to_vec(&cases).unwrap()).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let actual: Vec<[String; 2]> = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(actual, expected);
        eprintln!("matched {} upstream key-date pairs", cases.len());
    }
}
