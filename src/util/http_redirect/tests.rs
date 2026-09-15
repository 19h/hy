use std::io::Write;
use std::process::{Command, Stdio};

use reqwest::header::HeaderValue;
use serde_json::{Value, json};

use super::*;

#[test]
fn automatic_redirect_targets_match_httpx() {
    let mut cases = Vec::new();
    let mut expected = Vec::new();
    for base in [
        "http://user:pass@example.test:9000/start#keep",
        "https://example.test/start",
        "http://[::1]:9000/start#keep",
    ] {
        for locations in [
            vec!["https:/next"],
            vec!["HTTP:/next?q=one#new"],
            vec!["http:///next"],
            vec!["http:////next"],
            vec!["http://:8080/next"],
            vec!["http://:80/next"],
            vec!["https://user:pass@:8443/next"],
            vec!["https:/a/../next"],
            vec!["https:"],
            vec!["https:?q=one"],
            vec!["http:a/..//next"],
            vec!["http:./next"],
            vec!["/next"],
            vec!["/next#"],
            vec!["../next?q=one"],
            vec!["/one", "/two"],
            vec!["http:next"],
            vec!["http://:abc/next"],
            vec!["/next\tpath"],
            vec![],
        ] {
            for status in [200, 300, 301, 302, 303, 304, 307, 308] {
                let mut headers = HeaderMap::new();
                for location in &locations {
                    headers.append(header::LOCATION, HeaderValue::from_str(location).unwrap());
                }
                let result = target(&url::Url::parse(base).unwrap(), status, &headers);
                expected.push(match result {
                    Ok(url) => json!({"url": url.map(|url| url.to_string())}),
                    Err(_) => json!({"error": true}),
                });
                cases.push(json!({"base": base, "status": status, "locations": locations}));
            }
        }
    }
    assert_eq!(cases.len(), 480);
    let Some(python) = std::env::var_os("HY_TEST_HTTPX_ORACLE_PYTHON")
        .or_else(|| std::env::var_os("HY_TEST_BUNDLE_ORACLE_PYTHON"))
    else {
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
    assert!(output.status.success());
    let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
        assert_eq!(actual, expected, "case {}", cases[index]);
    }
}

#[test]
fn host_repair_discards_original_credentials_and_port_but_inherits_fragment() {
    let current = url::Url::parse("http://user:pass@example.test:9000/start#keep").unwrap();
    let headers =
        HeaderMap::from_iter([(header::LOCATION, HeaderValue::from_static("https:/next"))]);
    let repaired = target(&current, 302, &headers).unwrap().unwrap();
    assert_eq!(repaired.as_str(), "https://example.test/next#keep");
    let duplicate = HeaderValue::from_static("/one");
    let mut headers = HeaderMap::from_iter([(header::LOCATION, duplicate)]);
    headers.append(header::LOCATION, HeaderValue::from_static("/two"));
    assert_eq!(target(&current, 302, &headers).unwrap().unwrap().path(), "/one,%20/two");
}

#[test]
fn location_validation_applies_only_to_redirect_responses() {
    let current = url::Url::parse("https://example.test/start").unwrap();
    let prefix = "https://example.test/";
    for (location, valid) in [
        (format!("{prefix}{}", "a".repeat(65_536 - prefix.len())), true),
        (format!("{prefix}{}", "a".repeat(65_537 - prefix.len())), false),
        ("/next\tpath".into(), false),
        ("http:next".into(), false),
    ] {
        let headers =
            HeaderMap::from_iter([(header::LOCATION, HeaderValue::from_str(&location).unwrap())]);
        assert_eq!(target(&current, 302, &headers).is_ok(), valid);
        assert!(target(&current, 200, &headers).unwrap().is_none());
    }
}
