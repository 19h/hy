//! Compare upload lifecycle events using upstream methods and HTTPX MockTransport.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

pub(super) fn compare(file: &Path, cases: &[Value], expected: &[Value]) {
    let Some(python) = std::env::var_os("HY_TEST_UPLOAD_ORACLE_PYTHON") else {
        return;
    };
    let source =
        std::env::var_os("HY_TEST_HCLI_SOURCE").map(std::path::PathBuf::from).unwrap_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli"),
        );
    let script = r#"
import ast, asyncio, hashlib, io, json, os, sys
from pathlib import Path
from types import MethodType, SimpleNamespace
import httpx, pydantic
from rich.console import Console
from rich.progress import Progress, DownloadColumn, TransferSpeedColumn, TimeRemainingColumn
assert pydantic.__version__ == '2.12.5'
assert httpx.__version__ == '0.28.1'
data = json.load(sys.stdin)
root = Path(sys.argv[1]) / 'src/hcli/lib/api'
namespace = dict(globals(), BaseModel=pydantic.BaseModel,
    ENV=SimpleNamespace(HCLI_PORTAL_URL='https://portal.test', HCLI_API_URL='https://fixture.test'),
    stderr_console=Console(file=io.StringIO(), color_system=None))
future = ast.ImportFrom(module='__future__', names=[ast.alias(name='annotations')], level=0)
path = root / 'asset.py'
nodes = [node for node in ast.parse(path.read_text()).body
         if isinstance(node, ast.ClassDef) and node.name in ('AssetAPI', 'UploadResponse')]
exec(compile(ast.fix_missing_locations(ast.Module(body=[future] + nodes, type_ignores=[])), str(path), 'exec'), namespace)
path = root / 'common.py'
client = next(node for node in ast.parse(path.read_text()).body
              if isinstance(node, ast.ClassDef) and node.name == 'APIClient')
put = next(node for node in client.body if isinstance(node, ast.AsyncFunctionDef) and node.name == 'put_file')
exec(compile(ast.fix_missing_locations(ast.Module(body=[future, put], type_ignores=[])), str(path), 'exec'), namespace)

async def evaluate(ticket):
    events = []
    async def transport(request):
        events.append([request.method, request.url.path])
        assert await request.aread() == b'fixture'
        return httpx.Response(200, json={})
    async def post(url, payload):
        events.append(['POST', url])
        if len(events) == 1:
            assert payload['checksum'] == hashlib.sha256(b'fixture').hexdigest()
            return ticket
        assert payload == {}
        return {}
    async def handle(response):
        response.raise_for_status()
        return response
    async with httpx.AsyncClient(base_url='https://fixture.test', transport=httpx.MockTransport(transport)) as http:
        adapter = SimpleNamespace(client=http, post_json=post, _handle_response=handle)
        adapter.put_file = MethodType(namespace['put_file'], adapter)
        async def get_client():
            return adapter
        namespace['get_api_client'] = get_client
        success = True
        try:
            await namespace['AssetAPI']().upload_asset('shared', data['file'])
        except (AttributeError, TypeError, pydantic.ValidationError):
            success = False
    return {'success': success, 'events': events}

async def run():
    return [await evaluate(ticket) for ticket in data['cases']]
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
        .write_all(&serde_json::to_vec(&json!({"file": file, "cases": cases})).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(actual, expected, "{}", cases[index]);
    }
}
