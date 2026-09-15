use std::fs;
use std::sync::mpsc;

use serde_json::{Value, json};

use super::*;
use crate::error::Error;

fn fixture_cache(directory: &std::path::Path) -> cache::Cache {
    cache::Cache {
        path: directory.join("update_check.json"),
    }
}

#[test]
fn completed_checks_save_results_and_recent_cache_suppresses_another_check() {
    for latest in [None, Some("2.0.0")] {
        let directory = tempfile::tempdir().unwrap();
        let mut checker = BackgroundUpdateChecker::new();
        checker.start_with(
            fixture_cache(directory.path()),
            "1.0.0".into(),
            "fixture".into(),
            move || Ok(latest.map(str::to_owned)),
        );
        let expected = match latest {
            Some(latest) => {
                format!("\nUpdate available! 1.0.0 -> {latest}\nRun fixture update to update\n")
            }
            None => "\nYou have the latest version 1.0.0! ".into(),
        };
        assert_eq!(checker.get_result(Duration::from_secs(2)), Some(expected));
        let cache = fixture_cache(directory.path());
        let data: Value = serde_json::from_slice(&fs::read(&cache.path).unwrap()).unwrap();
        assert_eq!(data["latest_version"], json!(latest));
        assert!(data["last_check"].as_str().unwrap().ends_with("+00:00"));
        assert!(!cache.should_check(chrono::Utc::now().naive_utc()));

        let mut cached = BackgroundUpdateChecker::new();
        cached.start_with(cache, "1.0.0".into(), "fixture".into(), || panic!("recent cache"));
        assert_eq!(cached.get_result(Duration::ZERO), None);
    }
}

#[test]
fn failed_checks_preserve_the_cache_and_success_survives_cache_write_failure() {
    let directory = tempfile::tempdir().unwrap();
    let cache = fixture_cache(directory.path());
    let previous = br#"{"last_check":"2000-01-01T00:00:00+00:00","latest_version":"1.0.0"}"#;
    fs::write(&cache.path, previous).unwrap();
    let mut checker = BackgroundUpdateChecker::new();
    checker.start_with(cache, "1.0.0".into(), "hy".into(), || {
        Err(Error::Other("network unavailable".into()))
    });
    assert_eq!(checker.get_result(Duration::from_secs(2)), None);
    let cache = fixture_cache(directory.path());
    assert_eq!(fs::read(&cache.path).unwrap(), previous);
    assert!(cache.should_check(chrono::Utc::now().naive_utc()));

    let mut checker = BackgroundUpdateChecker::new();
    checker.start_with(fixture_cache(&cache.path), "1.0.0".into(), "hy".into(), || Ok(None));
    assert_eq!(
        checker.get_result(Duration::from_secs(2)),
        Some("\nYou have the latest version 1.0.0! ".into())
    );
    assert_eq!(fs::read(&cache.path).unwrap(), previous);
}

#[test]
fn completion_wakes_waiters_and_a_timeout_does_not_restart_the_worker() {
    let directory = tempfile::tempdir().unwrap();
    let mut checker = BackgroundUpdateChecker::new();
    let (release, wait) = mpsc::channel();
    checker.start_with(fixture_cache(directory.path()), "1.0.0".into(), "hy".into(), move || {
        wait.recv().unwrap();
        Ok(Some("2.0.0".into()))
    });
    assert_eq!(checker.get_result(Duration::from_millis(10)), None);
    checker.start_with(fixture_cache(directory.path()), "1.0.0".into(), "hy".into(), || {
        panic!("duplicate worker")
    });
    release.send(()).unwrap();
    let result = checker.get_result(Duration::from_secs(2));
    assert!(result.as_deref().unwrap().contains("1.0.0 -> 2.0.0"));
    assert_eq!(checker.get_result(Duration::ZERO), result);
}
