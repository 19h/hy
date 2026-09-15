"""Read-only filesystem comparisons against selected source helpers."""

import ast
import json
import os
import sys
from pathlib import Path
from types import SimpleNamespace

context = {"Path": Path, "os": os}
sources = {
    "src/hcli/lib/ida/python/__init__.py": {
        "_normalize_path", "_is_python_executable_name", "_get_venv_root_from_python",
    },
    "src/hcli/lib/ida/python/environment.py": {
        "_same_venv", "venv_executable_points_at", "is_homebrew_path",
    },
    "src/hcli/lib/venv.py": {"parse_pyvenv_cfg"},
}
context["HOMEBREW_PREFIXES"] = ("/opt/homebrew", "/usr/local/Cellar", "/home/linuxbrew/.linuxbrew")
for relative, names in sources.items():
    path = Path(sys.argv[1]) / relative
    nodes = [node for node in ast.parse(path.read_text()).body if getattr(node, "name", None) in names]
    future = ast.ImportFrom(module="__future__", names=[ast.alias(name="annotations")], level=0)
    tree = ast.fix_missing_locations(ast.Module(body=[future] + nodes, type_ignores=[]))
    exec(compile(tree, str(path), "exec"), context)
results = []
for case in json.load(sys.stdin):
    executable = Path(case["executable"])
    root = Path(case["root"])
    state = SimpleNamespace(idapython_venv_executable=executable, venv_root=root)
    results.append({
        "root": context["_get_venv_root_from_python"](str(executable)),
        "selects": context["venv_executable_points_at"](state),
        "config": context["parse_pyvenv_cfg"](root / "pyvenv.cfg"),
        "homebrew": context["is_homebrew_path"](executable),
    })
print(json.dumps(results, default=str))
