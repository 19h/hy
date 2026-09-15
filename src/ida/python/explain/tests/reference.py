"""Read-only source note/render oracle with an explicit native-runtime assumption."""
import ast
import dataclasses
import io
import json
import os
import platform
import sys
from pathlib import Path
from types import SimpleNamespace

from pydantic import BaseModel
from rich.console import Console
from rich.markup import escape

root = Path(sys.argv[1]) / "src/hcli"
context = {"__name__": "__main__", "BaseModel": BaseModel, "dataclass": dataclasses.dataclass,
    "Path": Path, "os": os, "sys": SimpleNamespace(prefix="/native-no-python-prefix"), "escape": escape,
    "Literal": __import__("typing").Literal, "platform": platform,
    "logger": SimpleNamespace(debug=lambda *args: None)}
future = ast.ImportFrom(module="__future__", names=[ast.alias(name="annotations")], level=0)
for path, select in [
    (root / "lib/venv.py", lambda node: getattr(node, "name", "") in {"parse_pyvenv_cfg", "get_python_exe_candidates", "find_virtual_env_python", "read_virtual_env_version", "get_virtual_env_version"}),
    (root / "lib/ida/python/__init__.py", lambda node: getattr(node, "name", "") in {"IdatProbe", "PythonVersionMismatch", "format_python_version_mismatch_warning", "_normalize_path", "_is_python_executable_name", "_get_venv_root_from_python", "find_python_version_mismatches"}),
    (root / "lib/ida/python/environment.py", lambda node: isinstance(node, ast.ClassDef) and (node.name.endswith("Report") or node.name in {"InstallationEntry", "CandidateVirtualEnv", "PythonVersionMismatchEntry", "EnvironmentNote"}) or getattr(node, "name", "") == "collect_notes"),
    (root / "commands/ida/python/explain_environment.py", lambda node: isinstance(node, ast.FunctionDef) and node.name != "explain_environment"),
]:
    nodes = [node for node in ast.parse(path.read_text()).body if select(node)]
    exec(compile(ast.fix_missing_locations(ast.Module(body=[future, *nodes], type_ignores=[])), str(path), "exec"), context)
for value in list(context.values()):
    if isinstance(value, type) and value is not BaseModel and issubclass(value, BaseModel):
        value.model_rebuild(_types_namespace=context)

results = []
for case in json.load(sys.stdin):
    if case["mode"] == "mismatches":
        context["probe_python_version"] = lambda path: case["versions"].get(str(path))
        probe = context["IdatProbe"].model_validate(case["probe"])
        mismatches = context["find_python_version_mismatches"](probe, Path(case["executable"]) if case["executable"] else None)
        results.append([dict(ida_version=item.ida_version, other_version=item.other_version,
            other_path=str(item.other_path), other_source=item.other_source) for item in mismatches])
        continue
    report = context["EnvironmentReport"].model_validate(case["report"])
    context["ENV"] = SimpleNamespace(HCLI_BINARY_NAME=case.get("binary", "hy"))
    if case["mode"] == "notes":
        notes = context["collect_notes"](report.python_environment, report.idapython_virtualenv, report.python_version)
        results.append([note.model_dump(mode="json") for note in notes])
    else:
        output = io.StringIO()
        context["console"] = Console(file=output, width=10000, color_system=None, force_terminal=False)
        context["render_environment_report_text"](report)
        results.append(output.getvalue())
print(json.dumps(results))
