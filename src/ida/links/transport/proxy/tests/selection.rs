use std::io::Write;
use std::process::{Command, Stdio};

use serde::Serialize;

use super::super::config::Config;

#[derive(Serialize)]
struct Case {
    entries: Vec<(String, String)>,
    target: String,
}
type Outcome = Result<Option<String>, ()>;

#[test]
fn environment_selection_matches_httpx() {
    let mut cases = Vec::new();
    let base = [
        ("HTTP_PROXY", "http://http.proxy:8000"),
        ("HTTPS_PROXY", "http://https.proxy:8001"),
        ("ALL_PROXY", "http://all.proxy:8002"),
    ];
    for bypass in [
        "",
        "*",
        "example.test",
        ".example.test",
        "localhost",
        "127.0.0.1",
        "127.0.0.1/8",
        "::1",
        "example.test:80",
        "example.test:8443",
        "https://example.test",
        "http://",
        "all://",
        "*.example.test",
        "other.test, example.test",
        "https://example.test:443",
        "HTTPS://example.test:443",
    ] {
        for target in [
            "http://example.test/",
            "https://example.test/",
            "https://example.test:8443/",
            "http://sub.example.test/",
            "http://notexample.test/",
            "http://localhost/",
            "http://sub.localhost/",
            "http://127.0.0.1/",
            "http://127.0.0.2/",
            "http://[::1]/",
        ] {
            let entries = base
                .iter()
                .map(|(key, value)| ((*key).into(), (*value).into()))
                .chain(std::iter::once(("NO_PROXY".into(), bypass.into())))
                .collect();
            cases.push(Case {
                entries,
                target: target.into(),
            });
        }
    }
    for extra in [
        vec![("http_proxy", "http://lower.proxy:8080")],
        vec![("http_proxy", "")],
        vec![("REQUEST_METHOD", "GET")],
        vec![("REQUEST_METHOD", "GET"), ("HTTP_proxy", "http://lower.proxy:8080")],
        vec![("HTTP_PROXY", "bad://proxy.test")],
        vec![("HTTP_PROXY", "bad://proxy.test"), ("NO_PROXY", "*")],
        vec![("HTTP_PROXY", "proxy.test:8080")],
        vec![("no_proxy", ""), ("NO_PROXY", "*")],
    ] {
        for target in ["http://example.test/", "https://example.test/"] {
            let entries = base
                .iter()
                .copied()
                .chain(extra.iter().copied())
                .map(|(key, value)| (key.into(), value.into()))
                .collect();
            cases.push(Case {
                entries,
                target: target.into(),
            });
        }
    }
    let actual: Vec<Outcome> = cases
        .iter()
        .map(|case| {
            Config::from_entries(case.entries.clone())
                .map(|config| {
                    config.select(&url::Url::parse(&case.target).unwrap()).map(|proxy| {
                        format!(
                            "{}://{}:{}",
                            proxy.url.scheme(),
                            proxy.url.host_str().unwrap(),
                            proxy.url.port_or_known_default().unwrap()
                        )
                    })
                })
                .map_err(|_| ())
        })
        .collect();
    assert_eq!(actual[0], Ok(Some("http://http.proxy:8000".into())));
    assert_eq!(actual[10], Ok(None));
    use sha2::{Digest, Sha256};
    let digest = format!("{:x}", Sha256::digest(serde_json::to_vec(&actual).unwrap()));
    assert_eq!(digest, "707d600fd18ad33e72b00b0a98740ba46bf939ef27fae6079948dceb650a34f3");
    eprintln!("proxy selection SHA-256 {digest}");
    let Some(python) = std::env::var_os("HY_TEST_HTTPX_ORACLE_PYTHON") else {
        return;
    };
    let mut child = Command::new(python).args(["-I", "-B", "-c", r#"
import json,sys,os,urllib.request,httpx
from unittest.mock import patch
from httpx._utils import get_environment_proxies,URLPattern
results=[]
for case in json.load(sys.stdin):
    with patch.dict(os.environ, dict(case['entries']), clear=True), patch('httpx._utils.getproxies', urllib.request.getproxies_environment):
        try:
            mounts={URLPattern(pattern): None if proxy is None else httpx.Proxy(proxy) for pattern,proxy in get_environment_proxies().items()}
            result=None
            for pattern,proxy in sorted(mounts.items()):
                if pattern.matches(httpx.URL(case['target'])):
                    if proxy is not None:
                        u=proxy.url
                        port=u.port if u.port is not None else (443 if u.scheme=='https' else 80)
                        result=f'{u.scheme}://{u.host}:{port}'
                    break
            results.append({'Ok':result})
        except (ValueError,httpx.InvalidURL): results.append({'Err':None})
json.dump(results,sys.stdout)
"#]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&cases).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let expected: Vec<Outcome> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.into_iter().zip(expected).enumerate() {
        assert_eq!(actual, expected, "proxy case {index}: {}", cases[index].target);
    }
    eprintln!("HTTPX proxy selection oracle matched {} cases", cases.len());
}
