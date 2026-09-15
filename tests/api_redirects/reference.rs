//! Execute upstream's PUT method with a real HTTPX redirect loop and mock I/O.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

pub(super) fn compare(cases: &[Value]) {
    let Some(python) = std::env::var_os("HY_TEST_UPLOAD_ORACLE_PYTHON") else {
        return;
    };
    let mut file = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
    file.write_all(b"fixture").unwrap();
    let source =
        std::env::var_os("HY_TEST_HCLI_SOURCE").map(std::path::PathBuf::from).unwrap_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli"),
        );
    let script = r#"
import ast, asyncio, io, json, os, sys
from pathlib import Path
import httpx
from rich.console import Console
from rich.progress import Progress, DownloadColumn, TransferSpeedColumn, TimeRemainingColumn
assert httpx.__version__ == '0.28.1'
data = json.load(sys.stdin)
path = Path(sys.argv[1]) / 'src/hcli/lib/api/common.py'
nodes = [node for node in ast.parse(path.read_text()).body if isinstance(node, ast.ClassDef)]
future = ast.ImportFrom(module='__future__', names=[ast.alias(name='annotations')], level=0)
namespace = dict(globals(), stderr_console=Console(file=io.StringIO(), color_system=None))
exec(compile(ast.fix_missing_locations(ast.Module(body=[future]+nodes, type_ignores=[])), str(path), 'exec'), namespace)
async def evaluate(case):
    events = []
    async def transport(request):
        payload = b''.join([chunk async for chunk in request.stream])
        events.append([request.method, request.url.path])
        assert payload == (b'fixture' if request.method == 'PUT' else b'')
        assert request.headers['content-type'] == 'application/json'
        if request.method == 'GET':
            assert 'content-length' not in request.headers
        if request.url.path == '/signed-put':
            return httpx.Response(case['status'], headers={'Location': '/redirected'} if case['location'] else {}, content=b'{}')
        return httpx.Response(200, content=b'{}')
    class StreamingTransport(httpx.AsyncBaseTransport):
        async def handle_async_request(self, request):
            return await transport(request)
    # MockTransport calls Request.aread(), replacing the generator with replayable
    # bytes. Iterate the stream directly to retain real-transport replay behavior.
    async with httpx.AsyncClient(transport=StreamingTransport()) as http:
        client = object.__new__(namespace['APIClient'])
        client.client = http
        success = True
        try:
            await client.put_file('https://fixture.test/signed-put', data['file'])
        except (httpx.HTTPError, httpx.StreamConsumed, namespace['APIError']):
            success = False
    return {'success': success, 'events': events}
async def run():
    return [await evaluate(case) for case in data['cases']]
print(json.dumps(asyncio.run(run())))
"#;
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", script])
        .arg(source)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&json!({"file":file.path(), "cases":cases})).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), cases.len());
    for (actual, case) in actual.iter().zip(cases) {
        assert_eq!(*actual, case["expected"], "{case}");
    }
}
