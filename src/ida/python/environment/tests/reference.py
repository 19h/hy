"""Read policy definitions from upstream; supply filesystem predicates as facts."""

import ast
import dataclasses
import json
import sys
from pathlib import Path
from types import SimpleNamespace

path = Path(sys.argv[1]) / "src/hcli/lib/ida/python/environment.py"
names = {
    "EnvironmentFinding", "PythonEnvironmentState", "SetupPattern",
    "get_recommended_venv_dir", "get_venv_python_path", "render_set_env_var_command",
    "_render_create_environment_hint", "check_python_environment", "identify_setup_pattern",
}
nodes = [node for node in ast.parse(path.read_text()).body if getattr(node, "name", None) in names]
future = ast.ImportFrom(module="__future__", names=[ast.alias(name="annotations")], level=0)
context = {
    "__name__": "__main__", "Path": Path, "dataclass": dataclasses.dataclass,
    "ENV": SimpleNamespace(HCLI_BINARY_NAME=sys.argv[2]),
    "logger": SimpleNamespace(debug=lambda *args: None),
    "_is_windows_store_shim": lambda path: "microsoft/windowsapps" in path.lower().replace("\\", "/"),
}
tree = ast.fix_missing_locations(ast.Module(body=[future] + nodes, type_ignores=[]))
exec(compile(tree, str(path), "exec"), context)
results = []
path_fields = {
    "python_exe", "idausr", "venv_root", "idapython_venv_executable", "shell_virtual_env",
    "idapythonrc_path", "base_prefix",
}
for case in json.load(sys.stdin):
    values = dict(case["state"])
    selects = values.pop("variable_selects_venv")
    homebrew = values.pop("homebrew")
    for key in path_fields:
        if values[key] is not None:
            values[key] = Path(values[key])
    state = context["PythonEnvironmentState"](**values)
    context["venv_executable_points_at"] = lambda state: selects
    context["is_homebrew_path"] = lambda path: homebrew
    results.append({
        "findings": [dataclasses.asdict(finding) for finding in context["check_python_environment"](state)],
        "pattern": dataclasses.asdict(context["identify_setup_pattern"](state)),
    })
print(json.dumps(results))
