//! Run upstream response classification with bodies that have not been pre-read.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::Value;

pub(super) fn compare(cases: &[Value]) {
    let Some(python) = std::env::var_os("HY_TEST_HTTPX_ORACLE_PYTHON") else {
        return;
    };
    let source =
        std::env::var_os("HY_TEST_HCLI_SOURCE").map(std::path::PathBuf::from).unwrap_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli"),
        );
    let script = r#"
import ast, asyncio, gzip, json, sys
from pathlib import Path
import httpx
from httpx._decoders import SUPPORTED_DECODERS
assert httpx.__version__ == '0.28.1'
assert set(SUPPORTED_DECODERS) == {'identity', 'gzip', 'deflate'}
path = Path(sys.argv[1]) / 'src/hcli/lib/api/common.py'
nodes = [node for node in ast.parse(path.read_text()).body if isinstance(node, ast.ClassDef)]
future = ast.ImportFrom(module='__future__', names=[ast.alias(name='annotations')], level=0)
namespace = {}
exec(compile(ast.fix_missing_locations(ast.Module(body=[future]+nodes, type_ignores=[])), str(path), 'exec'), namespace)
class Body(httpx.AsyncByteStream):
    def __init__(self, payload): self.payload = payload
    async def __aiter__(self): yield self.payload
async def evaluate(case):
    payload = b'broken gzip' if case['invalid'] else gzip.compress(b'{"message":"decoded failure"}')
    class Transport(httpx.AsyncBaseTransport):
        async def handle_async_request(self, request):
            assert request.headers['accept-encoding'] == 'gzip, deflate'
            return httpx.Response(case['status'], headers={'Content-Encoding':'gzip'}, stream=Body(payload))
    async with httpx.AsyncClient(transport=Transport()) as http:
        client = object.__new__(namespace['APIClient'])
        client.client = http
        try:
            if case['streamed']:
                async with http.stream('GET', 'https://fixture.test/content') as response:
                    await client._handle_response(response)
            else:
                await client.get_json('https://fixture.test/content', auth=False)
        except httpx.DecodingError:
            return 'decoding failed'
        except namespace['APIError'] as error:
            return str(error)
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
    let actual: Vec<String> = serde_json::from_slice(&output.stdout).unwrap();
    let expected: Vec<_> = cases.iter().map(|case| case["expected"].as_str().unwrap()).collect();
    assert_eq!(actual, expected);
}
