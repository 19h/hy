use std::io::Write;
use std::process::{Command, Stdio};

use reqwest::header::{HeaderMap, HeaderValue, SET_COOKIE};
use serde::Serialize;

use super::Jar;

mod legacy;
mod numbers;

const NOW: i64 = 1_800_000_000;
type Outcome = std::result::Result<Option<String>, ()>;

#[derive(Serialize)]
struct Store {
    source: String,
    headers: Vec<String>,
    cookie2: Vec<String>,
}

#[derive(Serialize)]
struct Case {
    stores: Vec<Store>,
    target: String,
    elapsed: i64,
    expected: Outcome,
}

impl Case {
    fn new(source: &str, headers: &[&str], target: &str, expected: Option<&str>) -> Self {
        Self {
            stores: vec![Store {
                source: source.into(),
                headers: headers.iter().map(|text| (*text).into()).collect(),
                cookie2: Vec::new(),
            }],
            target: target.into(),
            elapsed: 0,
            expected: Ok(expected.map(str::to_owned)),
        }
    }
}

fn assert_cases(cases: &[Case]) {
    for (index, case) in cases.iter().enumerate() {
        let mut jar = Jar::default();
        for store in &case.stores {
            let mut headers = HeaderMap::new();
            for value in &store.headers {
                headers.append(SET_COOKIE, HeaderValue::from_bytes(value.as_bytes()).unwrap());
            }
            for value in &store.cookie2 {
                headers.append("set-cookie2", HeaderValue::from_bytes(value.as_bytes()).unwrap());
            }
            jar.store_at(&headers, &url::Url::parse(&store.source).unwrap(), NOW);
        }
        let result = jar
            .header_at(&url::Url::parse(&case.target).unwrap(), NOW + case.elapsed)
            .map(|header| header.map(|value| value.to_str().unwrap().to_owned()))
            .map_err(|_| ());
        assert_eq!(result, case.expected, "case {index}: {}", case.target);
    }
    let Some(python) = std::env::var_os("HY_TEST_HTTPX_ORACLE_PYTHON") else {
        return;
    };
    const ORACLE: &str = r#"
import json, sys, httpx
from unittest.mock import patch
results = []
for case in json.load(sys.stdin):
    cookies = httpx.Cookies()
    with patch('http.cookiejar.time.time', return_value=1800000000) as clock:
        try:
            for store in case['stores']:
                request = httpx.Request('GET', store['source'], headers={'Host': 'original.test'})
                response = httpx.Response(200, request=request, headers=[
                    (b'set-cookie', value.encode('utf-8')) for value in store['headers']] + [
                    (b'set-cookie2', value.encode('utf-8')) for value in store['cookie2']])
                cookies.extract_cookies(response)
            clock.return_value += case['elapsed']
            request = httpx.Request('GET', case['target'])
            cookies.set_cookie_header(request)
            results.append({'Ok': request.headers.get('cookie')})
        except (UnicodeEncodeError, ValueError):
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
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(cases).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let outcomes: Vec<Outcome> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(outcomes.len(), cases.len());
    for (index, (case, actual)) in cases.iter().zip(outcomes).enumerate() {
        assert_eq!(actual, case.expected, "HTTPX case {index}: {}", case.target);
    }
    eprintln!("HTTPX cookie oracle matched {} cases", cases.len());
}

#[test]
fn domain_path_secure_and_port_rules_match_httpx() {
    let source = "https://example.test/dir/page";
    let mut cases = Vec::new();
    for (cookie, target, expected) in [
        ("sid=one", "https://example.test/dir/file", Some("sid=one")),
        ("sid=one", "https://sub.example.test/dir/file", Some("sid=one")),
        ("sid=one", "https://other.test/dir/file", None),
        ("sid=one", "https://example.test/dir", Some("sid=one")),
        ("sid=one", "https://example.test/directory", None),
        ("sid=one; Path=/", "https://example.test/elsewhere", Some("sid=one")),
        ("sid=one; Path=/dir/", "https://example.test/dir", None),
        ("sid=one; Path=/unrelated", "https://example.test/unrelated/file", Some("sid=one")),
        ("sid=one; Domain=.example.test", "https://sub.example.test/dir/file", Some("sid=one")),
        ("sid=one; Domain=other.test", "https://other.test/dir/file", None),
        ("sid=one; Domain=test", "https://example.test/dir/file", None),
        ("sid=one; Secure", "http://example.test/dir/file", None),
        ("sid=one; Secure", "https://example.test/dir/file", Some("sid=one")),
        ("sid=one; Secure=false", "http://example.test/dir/file", None),
        ("sid=one; Secure=", "http://example.test/dir/file", Some("sid=one")),
        ("sid=one; Port=80", "https://example.test/dir/file", Some("sid=one")),
        ("sid=one; Port=443", "https://example.test/dir/file", None),
        ("sid=one; Port", "https://example.test:8443/dir/file", None),
        ("sid=one; Port=80,invalid", "https://example.test/dir/file", Some("sid=one")),
        ("sid=one; Port=invalid,80", "https://example.test/dir/file", None),
        ("sid=one; Port=\"80\"", "https://example.test/dir/file", None),
        ("sid=one; Version=1", "https://example.test/dir", None),
        ("sid=one; Version=1", "https://example.test/dir/file", Some("sid=one")),
        ("sid=one; Version=2", "https://example.test/dir/file", None),
        ("sid=one; Version=-1", "https://example.test/dir/file", Some("sid=one")),
        ("sid=one; HttpOnly; SameSite=Strict", "https://example.test/dir/file", Some("sid=one")),
    ] {
        cases.push(Case::new(source, &[cookie], target, expected));
    }
    for (source, cookie, target, expected) in [
        ("http://127.0.0.1/", "sid=ip", "http://127.0.0.1:8080/", Some("sid=ip")),
        ("http://127.0.0.1/", "sid=ip; Domain=original.test", "http://127.0.0.1/", None),
        ("http://127.0.0.1/", "sid=ip", "http://original.test/", None),
        ("http://localhost/", "sid=local", "http://localhost/", Some("sid=local")),
        ("http://localhost/", "sid=local; Domain=localhost", "http://localhost/", None),
        ("http://[::1]:8080/", "sid=v6", "http://[::1]:9090/", Some("sid=v6")),
        ("http://[::1]:8080/", "sid=v6; Port", "http://[::1]:9090/", Some("sid=v6")),
        ("http://x.co.uk/", "sid=suffix; Domain=co.uk", "http://y.co.uk/", Some("sid=suffix")),
        (
            "http://example.test/a%2fb/page",
            "sid=escaped",
            "http://example.test/a%2Fb/file",
            Some("sid=escaped"),
        ),
    ] {
        cases.push(Case::new(source, &[cookie], target, expected));
    }
    assert_cases(&cases);
}

#[test]
fn expiry_and_attribute_precedence_match_httpx() {
    let url = "https://example.test/";
    let mut cases = Vec::new();
    for (cookie, expected) in [
        ("sid=one; Max-Age=0", None),
        ("sid=one; Max-Age=-1", None),
        ("sid=one; Max-Age=+1_000", Some("sid=one")),
        ("sid=one; Max-Age=invalid", None),
        ("sid=one; Expires=Wed, 09 Jun 2021 10:18:14 GMT", None),
        ("sid=one; Expires=Wed, 09 Jun 2038 10:18:14 GMT", Some("sid=one")),
        ("sid=one; Expires=invalid", Some("sid=one")),
        ("sid=one; Expires=invalid; Expires=Wed, 09 Jun 2021 10:18:14 GMT", None),
        ("sid=one; Max-Age=10; Expires=Wed, 09 Jun 2021 10:18:14 GMT", Some("sid=one")),
        ("sid=one; Expires=Wed, 09 Jun 2038 10:18:14 GMT; Max-Age=0", None),
        ("sid=one; Max-Age=0; Max-Age=10", Some("sid=one")),
        ("sid=one; Path=/; Path=/other", Some("sid=one")),
        ("sid=one; Domain", None),
        ("sid=one; Path", None),
        ("sid=one; Version=invalid", None),
        ("sid=one; Version=\"1\"", Some("sid=one")),
        ("flag", Some("flag")),
        ("empty=", Some("empty=")),
        ("=missing", None),
        ("quoted=\"one two\"", Some("quoted=\"one two\"")),
    ] {
        cases.push(Case::new(url, &[cookie], url, expected));
    }
    let mut expired = Case::new(url, &["sid=one; Max-Age=1"], url, None);
    expired.elapsed = 2;
    cases.push(expired);
    let mut non_ascii = Case::new(url, &["sid=é"], url, None);
    non_ascii.expected = Err(());
    cases.push(non_ascii);
    assert_cases(&cases);
}

#[test]
fn updates_deletions_and_header_order_match_httpx() {
    let url = "https://example.test/dir/file";
    let mut cases = vec![
        Case::new(url, &["a=old", "b=two", "a=new"], url, Some("a=new; b=two")),
        Case::new(
            url,
            &["a=root; Path=/", "b=two; Path=/dir", "a=deep; Path=/dir"],
            url,
            Some("b=two; a=deep; a=root"),
        ),
        Case::new(
            url,
            &["a=one", "b=two", "a=gone; Max-Age=0", "a=new"],
            url,
            Some("a=new; b=two"),
        ),
        Case::new(url, &["a=one", "a=gone; Max-Age=0"], url, Some("a=one")),
    ];
    let mut cross_source = Case::new(url, &["sid=original; Domain=example.test"], url, None);
    cross_source.stores.push(Store {
        source: "https://other.test/dir/file".into(),
        headers: vec!["sid=removed; Domain=example.test; Max-Age=0".into()],
        cookie2: Vec::new(),
    });
    cases.push(cross_source);
    assert_cases(&cases);
}

#[test]
fn legacy_expiry_dates_and_parse_failures_match_httpx() {
    let url = "https://example.test/";
    let mut cases = Vec::new();
    for (header, expected) in [
        ("sid=one; Expires=Wed, 01 Jan 1969 00:00:00 GMT", Some("sid=one")),
        ("sid=one; Expires=01 Jan 77", Some("sid=one")),
        ("sid=one; Expires=01 Jan 78", None),
        ("sid=one; Expires=01 Jan 1970 00:00 EST", Some("sid=one")),
        ("sid=one; Expires=01 Jan 1970", None),
        ("sid=one; Expires=\"01 Jan 1970", None),
        ("sid=one; Expires=01 Jan 1970\"", None),
        ("sid=one; Expires=Wed, 01 Jax 2038 00:00:00 GMT", None),
        ("sid=one; Max-Age=100; Expires=Wed, 01 Jax 2038 00:00:00 GMT", None),
        ("sid=one; Expires=01 Jan 2038; Expires=Wed, 01 Jax 2038 00:00:00 GMT", None),
    ] {
        cases.push(Case::new(url, &[header], url, expected));
    }
    let mut batch = Case::new(url, &["sid=original"], url, Some("sid=original"));
    batch.stores.push(Store {
        source: url.into(),
        cookie2: Vec::new(),
        headers: vec![
            "sid=deleted; Max-Age=0".into(),
            "new=pending".into(),
            "invalid=date; Expires=Wed, 01 Jax 2038 00:00:00 GMT".into(),
        ],
    });
    cases.push(batch);
    assert_cases(&cases);
}
