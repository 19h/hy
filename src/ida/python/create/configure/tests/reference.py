"""Execute only the reporting function, with in-memory configuration adapters."""

import ast
import io
import json
import sys
from pathlib import Path
from types import SimpleNamespace

from rich.console import Console
from rich.markup import escape


def build_configuration_plan(*args):
    return SimpleNamespace(
        steps=[], warnings=[], manual_instructions="manual", needs_logout=False
    )


def verify_env_var_in_subprocess(*args):
    return True


def step_results(steps):
    return [
        SimpleNamespace(
            step=SimpleNamespace(kind=kind),
            success=success,
            skipped=skipped,
            message="fixture",
        )
        for kind, success, skipped in steps
    ]


path = Path(sys.argv[1]) / "src/hcli/commands/ida/python/create_environment.py"
nodes = [
    node
    for node in ast.parse(path.read_text()).body
    if isinstance(node, ast.FunctionDef) and node.name == "configure_env_var"
]
future = ast.ImportFrom(module="__future__", names=[ast.alias(name="annotations")], level=0)
tree = ast.fix_missing_locations(ast.Module(body=[future] + nodes, type_ignores=[]))
context = {
    "ENV": SimpleNamespace(IDAPYTHON_VENV_EXECUTABLE=None),
    "ENV_VAR": "IDAPYTHON_VENV_EXECUTABLE",
    "console": Console(file=io.StringIO(), color_system=None),
    "stderr_console": Console(file=io.StringIO(), color_system=None),
    "escape": escape,
    "build_configuration_plan": build_configuration_plan,
    "verify_env_var_in_subprocess": verify_env_var_in_subprocess,
}
exec(compile(tree, str(path), "exec"), context)
results = []
for case in json.load(sys.stdin):
    steps = step_results(case["steps"])
    context["execute_configuration_plan"] = lambda plan: steps
    results.append(
        context["configure_env_var"](Path("/fixture/python"), interactive=False, quiet=True)
    )
print(json.dumps(results))
