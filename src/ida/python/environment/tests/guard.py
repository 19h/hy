"""Read-only warning/exception formatter comparison."""

import ast
import io
import json
import sys
from pathlib import Path
from types import SimpleNamespace

from rich.console import Console
from rich.markup import escape

path = Path(sys.argv[1]) / "src/hcli/lib/ida/python/environment.py"
names = {"format_environment_warnings", "format_environment_findings_plain"}
nodes = [node for node in ast.parse(path.read_text()).body if getattr(node, "name", None) in names]
future = ast.ImportFrom(module="__future__", names=[ast.alias(name="annotations")], level=0)
context = {"escape": escape}
tree = ast.fix_missing_locations(ast.Module(body=[future] + nodes, type_ignores=[]))
exec(compile(tree, str(path), "exec"), context)
results = []
for case in json.load(sys.stdin):
    context["ENV"] = SimpleNamespace(HCLI_BINARY_NAME=case["binary"])
    findings = [SimpleNamespace(**finding) for finding in case["findings"]]
    warning = context["format_environment_warnings"](findings)
    output = io.StringIO()
    if warning:
        Console(file=output, color_system=None, width=10000).print(warning, highlight=False, end="")
    results.append({
        "warning": output.getvalue(),
        "error": "HCLI cannot install plugin dependencies into IDA's Python environment:\n"
        + context["format_environment_findings_plain"](findings),
    })
print(json.dumps(results))
