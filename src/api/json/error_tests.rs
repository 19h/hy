use serde_json::{Value, json};

use crate::error::Error;

fn documents() -> Vec<String> {
    let mut documents: Vec<_> = [
        "null",
        "true",
        "false",
        "0",
        "-0",
        "-0.0",
        "1.0",
        "1e-5",
        "1e20",
        "1e9999",
        "-1e9999",
        "-2.0156234722508763e14",
        "[]",
        "{}",
        "[null,true,false,1.0]",
        r#""plain\ntext""#,
        r#"["it's", "café-🦀", "\u00a0\u200b"]"#,
        r#"{"z":null,"a":[true,1.0],"z":false}"#,
    ]
    .into_iter()
    .map(|message| format!("{{\"message\":{message}}}"))
    .collect();
    documents.extend(
        ["null", "[]", "{}", "true", "42", "[\"message\"]", "\"message\"", "invalid JSON"]
            .map(String::from),
    );
    documents.push(format!("{{\"message\":{}}}", "1".repeat(4300)));
    documents.push(format!("{{\"message\":{}}}", "1".repeat(4301)));
    documents.push(format!("{{\"message\":true,\"unused\":{}}}", "1".repeat(4301)));
    documents
}

#[test]
fn status_messages_match_actual_upstream_exception_arguments() {
    let mut cases = Vec::new();
    for status in [400, 401, 403, 404, 418, 429, 500] {
        for document in documents() {
            let (class, message) = match super::status_error(status, document.as_bytes()) {
                Error::Authentication(message) | Error::Forbidden(message) => {
                    ("AuthenticationError", message)
                }
                Error::NotFound(message) => ("NotFoundError", message),
                Error::RateLimit => ("RateLimitError", "Rate limit exceeded".into()),
                Error::Api {
                    message,
                    ..
                } => ("APIError", message),
                error => panic!("unexpected error: {error}"),
            };
            cases.push(json!({"status":status, "document":document, "expected":[class, message]}));
        }
    }
    assert_eq!(cases.len(), 203);
    let bytes = serde_json::to_vec(&cases.iter().map(|case| &case["expected"]).collect::<Vec<_>>())
        .unwrap();
    use sha2::Digest;
    assert_eq!(
        format!("{:x}", sha2::Sha256::digest(&bytes)),
        "ef33b340348471e1cb406df7662de61b1851ce56b680156f2440b220a558a177"
    );
    compare(&cases);
}

fn compare(cases: &[Value]) {
    let Some(python) = std::env::var_os("HY_TEST_JSON_ORACLE_PYTHON") else {
        return;
    };
    use std::io::Write;
    use std::path::Path;
    use std::process::{Command, Stdio};
    let source =
        std::env::var_os("HY_TEST_HCLI_SOURCE").map(std::path::PathBuf::from).unwrap_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli"),
        );
    let script = r#"
import ast, asyncio, json, sys
from pathlib import Path
import httpx
assert httpx.__version__ == '0.28.1'
assert sys.version_info[:3] == (3, 13, 15)
path = Path(sys.argv[1]) / 'src/hcli/lib/api/common.py'
nodes = [node for node in ast.parse(path.read_text()).body if isinstance(node, ast.ClassDef)]
future = ast.ImportFrom(module='__future__', names=[ast.alias(name='annotations')], level=0)
namespace = {}
exec(compile(ast.fix_missing_locations(ast.Module(body=[future]+nodes, type_ignores=[])), str(path), 'exec'), namespace)
async def evaluate(case):
    client = object.__new__(namespace['APIClient'])
    response = httpx.Response(case['status'], content=case['document'].encode('utf-8'))
    try:
        await client._handle_response(response)
    except namespace['APIError'] as error:
        return [type(error).__name__, str(error)]
    raise AssertionError('fixture must fail')
async def run(cases): return [await evaluate(case) for case in cases]
print(json.dumps(asyncio.run(run(json.load(sys.stdin)))))
"#;
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", script])
        .arg(source)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(cases).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), cases.len());
    for (actual, case) in actual.iter().zip(cases) {
        assert_eq!(*actual, case["expected"], "{case}");
    }
}
