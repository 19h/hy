//! Run actual upstream orchestration with in-memory creation and pip adapters.

use serde_json::Value;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

pub(super) fn compare(cases: &[Value]) {
    let Some(python) = std::env::var_os("HY_TEST_VENV_ORACLE_PYTHON") else {
        return;
    };
    let source =
        std::env::var_os("HY_TEST_HCLI_SOURCE").map(std::path::PathBuf::from).unwrap_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli"),
        );
    let script = r#"
import ast, io, json, logging, sys
from pathlib import Path
from types import SimpleNamespace
import rich.status
import rich_click as click
from rich.console import Console
from rich.markup import escape
from rich.prompt import Confirm
from pydantic import BaseModel
root = Path(sys.argv[1]) / 'src/hcli'
future = ast.ImportFrom(module='__future__', names=[ast.alias(name='annotations')], level=0)
for path, names in [(root / 'lib/ida/python/environment.py', {'render_set_env_var_command'}),
    (root / 'commands/ida/python/create_environment.py', None)]:
    nodes = [node for node in ast.parse(path.read_text()).body if isinstance(node, (ast.ClassDef, ast.FunctionDef))
        and (node.name in names if names is not None else node.name != 'create_environment')]
    exec(compile(ast.fix_missing_locations(ast.Module(body=[future]+nodes, type_ignores=[])), str(path), 'exec'), globals())
ENV = SimpleNamespace(HCLI_BINARY_NAME='hy', IDAPYTHON_VENV_EXECUTABLE=None)
ENV_VAR = 'IDAPYTHON_VENV_EXECUTABLE'
logger = logging.getLogger('oracle')
console = stderr_console = Console(file=io.StringIO(), color_system=None)
get_system = lambda: 'macos'
determine_target_python_version = lambda value: SimpleNamespace(version='3.13', source='--python-version', probe=None)
get_registered_python_exe = lambda probe: None
find_uv = lambda: Path('/fixture/uv')
plan_virtual_environment = lambda *args, **kwargs: SimpleNamespace(tool='uv', render_command=lambda: 'fixture uv')
results = []
for case in json.load(sys.stdin):
    target = Path(case['target'])
    executable = target / 'bin/python'
    inspect_target = lambda *args: SimpleNamespace(kind='healthy-venv' if case['existing'] else 'missing', python_exe=executable)
    create_virtual_environment = lambda *args: executable
    collect_plugin_dependencies = lambda: [SimpleNamespace(name='example', dependencies=[case['dependency']])] if case['dependency'] else []
    def install_single_plugin_dependencies(exe, plugin):
        success = plugin.dependencies != ['faildep']
        return SimpleNamespace(name=plugin.name, dependencies=plugin.dependencies, success=success, error=None if success else 'fixture failure')
    result = run_create_environment(path=target, python_version='3.13', configure=False,
        reinstall_plugins=not case['skip'], interactive=False, quiet=True).model_dump(mode='json')
    for migration in result['plugin_migrations']: migration['error'] = migration['error'] is not None
    results.append(result)
print(json.dumps(results))
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
