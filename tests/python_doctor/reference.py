"""Compare plain report rendering without collecting or modifying the environment."""

import ast
import io
import json
import sys
from pathlib import Path
from types import SimpleNamespace

from rich.console import Console
from rich.markup import escape


def record(value):
    if isinstance(value, dict):
        return SimpleNamespace(**{key: record(item) for key, item in value.items()})
    if isinstance(value, list):
        return [record(item) for item in value]
    return value


path = Path(sys.argv[1]) / "src/hcli/commands/ida/python/doctor.py"
names = {"_kv", "_render_finding", "render_doctor_report_text"}
nodes = [node for node in ast.parse(path.read_text()).body if getattr(node, "name", None) in names]
future = ast.ImportFrom(module="__future__", names=[ast.alias(name="annotations")], level=0)
context = {"escape": escape, "ENV": SimpleNamespace(HCLI_BINARY_NAME="hy")}
tree = ast.fix_missing_locations(ast.Module(body=[future] + nodes, type_ignores=[]))
exec(compile(tree, str(path), "exec"), context)
results = []
for case in json.load(sys.stdin):
    output = io.StringIO()
    context["ENV"] = SimpleNamespace(HCLI_BINARY_NAME=case["binary"])
    context["console"] = Console(file=output, color_system=None, width=10000)
    context["render_doctor_report_text"](record(case["report"]))
    results.append(output.getvalue())
print(json.dumps(results))
