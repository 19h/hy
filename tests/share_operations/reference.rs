//! Execute the pinned upstream command bodies with owned, in-memory API adapters.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

pub(super) fn verify_lookup_errors() {
    let Some(python) = std::env::var_os("HY_TEST_SHARE_REPORT_ORACLE_PYTHON") else {
        return;
    };
    let source =
        std::env::var_os("HY_TEST_HCLI_SOURCE").map(std::path::PathBuf::from).unwrap_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli"),
        );
    let script = r#"
import ast, asyncio, json, sys, httpx, pydantic
from pathlib import Path
assert pydantic.__version__ == '2.12.5'
assert httpx.__version__ == '0.28.1'
root = Path(sys.argv[1]) / 'src/hcli/lib/api'
namespace = dict(globals(), BaseModel=pydantic.BaseModel)
future = ast.ImportFrom(module='__future__', names=[ast.alias(name='annotations')], level=0)
for name in ['common', 'asset']:
    path = root / (name + '.py')
    nodes = [node for node in ast.parse(path.read_text()).body if isinstance(node, ast.ClassDef)
             and (name == 'common' or node.name in ['Asset', 'AssetAPI'])]
    exec(compile(ast.fix_missing_locations(ast.Module(body=[future]+nodes, type_ignores=[])), str(path), 'exec'), namespace)
async def evaluate(status):
    async def transport(request):
        return httpx.Response(status, json={'key':'missing-filename'})
    async with httpx.AsyncClient(base_url='https://fixture.test', transport=httpx.MockTransport(transport)) as http:
        client = object.__new__(namespace['APIClient'])
        client.client = http
        client._get_headers = lambda auth=True: {}
        async def get_client():
            return client
        namespace['get_api_client'] = get_client
        try:
            await namespace['AssetAPI']().get_shared_file_by_code('fixture-code')
        except Exception as error:
            return type(error).__name__
        return 'returned'
async def run():
    return [await evaluate(status) for status in [401, 403, 404, 429, 500, 200]]
print(json.dumps(asyncio.run(run())))
"#;
    let output =
        Command::new(python).args(["-I", "-B", "-c", script]).arg(source).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let actual: Vec<String> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        actual,
        [
            "AuthenticationError",
            "AuthenticationError",
            "NotFoundError",
            "RateLimitError",
            "APIError",
            "ValidationError"
        ]
    );
}

pub(super) fn compare(
    operation: &str,
    asset: Value,
    target: &Path,
    output: &std::process::Output,
    requests: usize,
) {
    let Some(python) = std::env::var_os("HY_TEST_SHARE_REPORT_ORACLE_PYTHON") else {
        return;
    };
    let source =
        std::env::var_os("HY_TEST_HCLI_SOURCE").map(std::path::PathBuf::from).unwrap_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli"),
        );
    let script = r#"
import ast, asyncio, contextlib, importlib.metadata, io, json, sys
from pathlib import Path
from types import SimpleNamespace
import pydantic, rich_click as click
from rich.console import Console
assert pydantic.__version__ == '2.12.5'
assert importlib.metadata.version('click') == '8.1.8'
assert importlib.metadata.version('rich') == '14.3.2'
assert importlib.metadata.version('rich-click') == '1.9.7'
data = json.load(sys.stdin)
root = Path(sys.argv[1])
model_path = root / 'src/hcli/lib/api/asset.py'
nodes = [node for node in ast.parse(model_path.read_text()).body
         if isinstance(node, ast.ClassDef) and node.name == 'Asset']
models = {'BaseModel': pydantic.BaseModel}
exec(compile(ast.Module(body=nodes, type_ignores=[]), str(model_path), 'exec'), models)
events = []
async def lookup(code):
    events.append('lookup')
    return models['Asset'](**data['asset'])
async def download(*args, **kwargs):
    events.append('download')
    return Path(data['target'])
async def delete(*args):
    events.append('delete')
async def get_client():
    return SimpleNamespace(download_file=download)
report = io.StringIO()
console = Console(file=report, color_system=None, width=10000, highlight=False)
namespace = {'Path': Path, 'click': click, 'console': console, 'stderr_console': console,
             'asset': SimpleNamespace(get_shared_file_by_code=lookup, delete_file_by_key=delete),
             'get_api_client': get_client, 'SHARED': 'shared'}
path = root / ('src/hcli/commands/share/' + data['operation'] + '.py')
nodes = [node for node in ast.parse(path.read_text()).body
         if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))]
for node in nodes:
    node.decorator_list = []
exec(compile(ast.Module(body=nodes, type_ignores=[]), str(path), 'exec'), namespace)
success = True
with contextlib.redirect_stdout(report):
    try:
        if data['operation'] == 'get':
            asyncio.run(namespace['get']('fixture-code', None, Path(data['target']), True))
        else:
            asyncio.run(namespace['delete']('fixture-code', True))
    except click.Abort:
        success = False
print(json.dumps({'success': success, 'report': report.getvalue(), 'requests': len(events)}))
"#;
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", script])
        .arg(source)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let input = json!({"operation": operation, "asset": asset, "target": target});
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&input).unwrap()).unwrap();
    let reference = child.wait_with_output().unwrap();
    assert!(reference.status.success());
    let reference: Value = serde_json::from_slice(&reference.stdout).unwrap();
    assert_eq!(reference["success"], output.status.success());
    assert_eq!(reference["requests"], requests);
    let report = String::from_utf8_lossy(&output.stdout);
    if output.status.success() {
        assert_eq!(reference["report"], report.as_ref());
    } else {
        // Parser/transport diagnostics differ; compare the side-effect boundary.
        assert_eq!(
            reference["report"].as_str().unwrap().contains("File downloaded successfully"),
            report.contains("File downloaded successfully")
        );
    }
}
