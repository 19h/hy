use std::io::Write;
use std::process::{Command, Stdio};

use super::*;

#[test]
fn registry_server_grammar_preserves_partial_results_and_socks_fallback() {
    let cases: &[(&str, &[(&str, &str)])] = &[
        (
            "proxy:80",
            &[
                ("http", "http://proxy:80"),
                ("https", "http://proxy:80"),
                ("ftp", "http://proxy:80"),
            ],
        ),
        (
            "https=secure:443;http=plain:80",
            &[("https", "http://secure:443"), ("http", "http://plain:80")],
        ),
        ("http=https://secure:443", &[("http", "https://secure:443")]),
        ("http=first;broken;https=last", &[("http", "http://first")]),
        ("socks=proxy:1080;", &[("socks", "socks://proxy:1080")]),
        (
            "socks=proxy:1080",
            &[
                ("socks", "socks://proxy:1080"),
                ("http", "socks4://proxy:1080"),
                ("https", "socks4://proxy:1080"),
            ],
        ),
        (
            "socks=proxy;http=plain",
            &[("socks", "socks://proxy"), ("http", "http://plain"), ("https", "socks4://proxy")],
        ),
        ("HTTP=proxy;http=first;http=last", &[("HTTP", "proxy"), ("http", "http://last")]),
        ("http=one; https=two", &[("http", "http://one"), (" https", "two")]),
        ("broken;http=ignored", &[]),
    ];
    for (server, expected) in cases {
        let expected =
            expected.iter().map(|(key, value)| ((*key).into(), (*value).into())).collect();
        assert_eq!(parse(server), expected, "{server}");
    }
}

#[test]
fn registry_parser_matches_cpython_when_oracle_is_available() {
    let Some(python) = std::env::var_os("HY_TEST_HTTPX_ORACLE_PYTHON") else {
        return;
    };
    let mut cases: Vec<String> =
        ["", "proxy:80", "http=one;broken;https=two", "socks=one;", ";", "http=one;http=two"]
            .into_iter()
            .map(str::to_owned)
            .collect();
    for scheme in ["http", "https", "ftp", "socks", "HTTP", "", "other"] {
        for address in [
            "proxy:80",
            "",
            "https://proxy",
            "socks://proxy",
            "socks5://proxy",
            "://proxy",
            "a/b://proxy",
            "a:b://proxy",
            "a b://proxy",
        ] {
            cases.push(format!("{scheme}={address}"));
            cases.push(format!("{scheme}={address};socks=fallback"));
        }
    }
    // Execute the installed standard-library function with a read-only registry
    // fixture. No platform settings or Python source files are modified.
    let script = r#"
import ast, json, re, sys, types, urllib.request
from pathlib import Path
tree = ast.parse(Path(urllib.request.__file__).read_text())
function = next(node for node in ast.walk(tree) if isinstance(node, ast.FunctionDef) and node.name == 'getproxies_registry')
class Key:
    def Close(self): pass
registry = types.ModuleType('winreg')
registry.HKEY_CURRENT_USER = 0
registry.OpenKey = lambda *args: Key()
sys.modules['winreg'] = registry
scope = {'re': re}
exec(compile(ast.Module(body=[function], type_ignores=[]), '<stdlib-registry-oracle>', 'exec'), scope)
results = []
for server in json.load(sys.stdin):
    registry.QueryValueEx = lambda key, name: (1 if name == 'ProxyEnable' else server, 0)
    results.append(scope['getproxies_registry']())
print(json.dumps(results))
"#;
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start CPython registry parser oracle");
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&cases).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let expected: Vec<ProxyMap> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(expected.len(), cases.len());
    for (server, expected) in cases.iter().zip(expected) {
        assert_eq!(parse(server), expected, "{server}");
    }
    eprintln!("matched {} CPython registry proxy cases", cases.len());
}
