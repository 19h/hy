//! Date syntax and command-specific formatting through isolated CLI reports.

mod support;

use std::fs;

use serde_json::json;
use support::auth::{command, stored, write_config};
use support::http::{Response, Server};
use support::{Sandbox, assert_success};

#[test]
fn credential_reports_fall_back_as_a_pair_and_key_reports_use_relative_time() {
    let sandbox = Sandbox::new();
    let mut config = stored("key", "fixture-key");
    config["hcli.credentials"]["credentials"]["account"]["created_at"] = "20260915T123456Z".into();
    config["hcli.credentials"]["credentials"]["account"]["last_used"] = "invalid".into();
    write_config(&sandbox, &config);
    let before = fs::read(sandbox.config_path()).unwrap();
    let last_used = (chrono::Utc::now() - chrono::TimeDelta::hours(49)).to_rfc3339();
    let server = Server::start(move |request, _| {
        assert_eq!(request.path, "/api/keys");
        Response::json(json!([
            {"name": "used", "created_at": "2026W382T123456Z", "last_used_at": last_used, "request_count": 1},
            {"name": "unused", "created_at": "invalid", "last_used_at": null, "request_count": 0}
        ]))
    });
    let output = command(&sandbox, &server, &["auth", "list"]).output().unwrap();
    assert_success(&output);
    let text = String::from_utf8_lossy(&output.stderr);
    assert!(text.contains("20260915T123456Z"), "{text}");
    assert!(text.contains("invalid"));
    assert_eq!(fs::read(sandbox.config_path()).unwrap(), before);
    assert!(server.requests().is_empty());

    let output = command(&sandbox, &server, &["auth", "key", "list"]).output().unwrap();
    assert_success(&output);
    let text = String::from_utf8_lossy(&output.stderr);
    for expected in ["Sep 15 2026", "2 days ago", "Unknown", "never"] {
        assert!(text.contains(expected), "{text}");
    }
}

#[test]
fn shared_file_dates_keep_seconds_and_raw_invalid_values() {
    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("key", "fixture-key"));
    let server = Server::start(|request, _| {
        assert!(request.path.starts_with("/api/assets/shared?"));
        Response::json(json!({"offset": 0, "limit": 20, "total": 2, "items": [
            {"filename": "a", "key": "a", "created_at": "2026-W38-2T123456+02", "size": 1, "version": 1},
            {"filename": "b", "key": "b", "created_at": "invalid-date-value", "size": 1, "version": 1}
        ]}))
    });
    let output =
        command(&sandbox, &server, &["share", "list", "--no-interactive"]).output().unwrap();
    assert_success(&output);
    let text = String::from_utf8_lossy(&output.stderr);
    assert!(text.contains("2026-09-15 12:34:56"), "{text}");
    assert!(text.contains("invalid-date-value"), "{text}");
}

#[test]
fn license_reports_interpret_aware_basic_dates_and_preserve_naive_dates() {
    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("key", "fixture-key"));
    let end = chrono::Utc::now() + chrono::TimeDelta::hours(49);
    let basic = end.format("%Y%m%dT%H%M%SZ").to_string();
    let server = Server::start(move |request, _| match request.path.as_str() {
        "/api/customers" => Response::json(json!([{"id": 17}])),
        "/api/licenses/17?page=1&limit=100" => Response::json(json!({"total": 2, "items": [
            {"pubhash": "aware", "product_catalog": "subscription", "status": "active", "end_date": basic, "addons": []},
            {"pubhash": "naive", "product_catalog": "subscription", "status": "active", "end_date": "2026-09-15", "addons": []}
        ]})),
        _ => Response::missing(),
    });
    let output = command(&sandbox, &server, &["license", "list"]).output().unwrap();
    assert_success(&output);
    let text = String::from_utf8_lossy(&output.stderr);
    assert!(text.contains("2d"), "{text}");
    assert!(text.contains("2026-09-15"), "{text}");
}
