"""Derive interpreters from Rust-owned fixture files without executing an interpreter."""

import ast
import contextlib
import io
import json
import os
import platform
import sys
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

source = Path(sys.argv[1]) / "src/hcli/lib"
probe_tree = ast.parse((source / "ida/python/__init__.py").read_text())
source_probe = next(
    ast.literal_eval(node.value) for node in probe_tree.body
    if isinstance(node, ast.Assign) and isinstance(node.targets[0], ast.Name)
    and node.targets[0].id == "GET_PYTHON_INFO_PY"
)

def run_probe(payload):
    output = io.StringIO()
    with contextlib.redirect_stdout(output):
        try:
            exec(payload, {})
        except SystemExit as error:
            assert error.code is None
    return json.loads(output.getvalue().split(":", 1)[1])

for frozen in (False, True):
    for executable in (None, sys.executable):
        with patch.object(sys, "frozen", frozen, create=True), patch.object(sys, "executable", executable):
            actual, expected = run_probe(sys.argv[2]), run_probe(source_probe)
        for field in ("frozen", "executable", "prefix", "base_prefix", "virtual_env", "idapython_venv_executable", "externally_managed"):
            assert actual[field] == expected[field], (field, actual, expected)
        assert actual["version"] == f'{expected["version_major"]}.{expected["version_minor"]}'

context = {
    "os": os, "platform": platform, "Path": Path,
    "logger": SimpleNamespace(debug=lambda *args: None),
    "ENV": SimpleNamespace(HCLI_BINARY_NAME="fixture-hy"),
}
future = ast.ImportFrom(module="__future__", names=[ast.alias(name="annotations")], level=0)
for path, names in [
    (source / "venv.py", {"get_python_exe_candidates"}),
    (source / "ida/python/__init__.py", {
        "PythonNotFoundError", "_normalize_path", "_is_windows_store_shim",
        "_is_python_executable_name", "_get_venv_root_from_python", "_derive_python_exe",
    }),
]:
    nodes = [node for node in ast.parse(path.read_text()).body if getattr(node, "name", None) in names]
    module = ast.fix_missing_locations(ast.Module(body=[future, *nodes], type_ignores=[]))
    exec(compile(module, str(path), "exec"), context)

results = []
for case in json.load(sys.stdin):
    info = dict(case["info"])
    major, minor = info.pop("version").split(".")
    info.update(version_major=int(major), version_minor=int(minor))
    try:
        results.append({"path": str(context["_derive_python_exe"](SimpleNamespace(**info)))})
    except context["PythonNotFoundError"] as error:
        results.append({"error": str(error)})
print(json.dumps(results))
