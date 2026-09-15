use chrono::NaiveDate;
use serde_json::json;

use super::*;

#[test]
fn update_cache_uses_aware_timestamps_and_a_strict_one_day_boundary() {
    let now = NaiveDate::from_ymd_opt(2026, 9, 15).unwrap().and_hms_opt(12, 0, 0).unwrap();
    let cases = [
        (json!({}), true),
        (json!([]), true),
        (json!(null), true),
        (json!({"last_check": null}), true),
        (json!({"last_check": 123}), true),
        (json!({"last_check": "invalid"}), true),
        (json!({"last_check": "2026-09-15"}), true),
        (json!({"last_check": "2026-09-15T12:00:00"}), true),
        (json!({"last_check": "2026-09-14T11:59:59.999999Z"}), true),
        (json!({"last_check": "2026-09-14T12:00:00Z"}), false),
        (json!({"last_check": "2026-09-14T12:00:00.000001Z"}), false),
        (json!({"last_check": "2026-09-15T12:00:00+00:00"}), false),
        (json!({"last_check": "20260914T140000+0200"}), false),
        (json!({"last_check": "2026-W38-1T120000Z"}), false),
        (json!({"last_check": "2026-09-16T00:00:00Z"}), false),
        (json!({"last_check": "2026-09-15T12:00:00+00:00Z"}), true),
    ];
    for (value, should_check) in &cases {
        assert_eq!(!is_recent(value, now), *should_check, "{value}");
    }

    if let Some(python) = std::env::var_os("HY_TEST_UPDATE_ORACLE_PYTHON") {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut child = Command::new(python).args(["-I", "-B", "-c", r#"
import datetime, json, sys
from pathlib import Path
from unittest.mock import mock_open, patch
import hcli.lib.update.version as upstream
class Clock(datetime.datetime):
    @classmethod
    def now(cls, tz=None):
        return cls(2026, 9, 15, 12, 0, tzinfo=tz)
checker = object.__new__(upstream.BackgroundUpdateChecker)
checker.cache_enabled = True
checker.cache_file = Path('/unused/update_check.json')
checker.check_interval = datetime.timedelta(hours=24)
results = []
for data in json.load(sys.stdin):
    with patch.object(upstream, 'datetime', Clock), patch.object(Path, 'exists', return_value=True), patch('builtins.open', mock_open(read_data=json.dumps(data))):
        results.append(checker.should_check())
        assert checker._load_cached_result() is None
assert upstream.compare_versions('1.0.0', '2.0.0') is False
print(json.dumps(results))
"#]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(
                &serde_json::to_vec(&cases.iter().map(|(value, _)| value).collect::<Vec<_>>())
                    .unwrap(),
            )
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        assert_eq!(
            serde_json::from_slice::<Vec<bool>>(&output.stdout).unwrap(),
            cases.iter().map(|(_, expected)| *expected).collect::<Vec<_>>()
        );
    }
}

#[test]
fn missing_malformed_or_unreadable_cache_does_not_suppress_discovery() {
    let directory = tempfile::tempdir().unwrap();
    let cache = Cache {
        path: directory.path().join("update_check.json"),
    };
    let now = Utc::now().naive_utc();
    assert!(cache.should_check(now));
    std::fs::write(&cache.path, b"{malformed").unwrap();
    assert!(cache.should_check(now));
    std::fs::remove_file(&cache.path).unwrap();
    std::fs::create_dir(&cache.path).unwrap();
    assert!(cache.should_check(now));
}
