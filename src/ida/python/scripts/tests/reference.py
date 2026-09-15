"""Read-only source oracle for script framing and missing-script diagnostics."""

import ast
import dataclasses
import json
import sys
from pathlib import Path
from types import SimpleNamespace

path = Path(sys.argv[1]) / "src/hcli/lib/ida/python/__init__.py"
tree = ast.parse(path.read_text())
request = json.load(sys.stdin)
constants = {
    node.targets[0].id: ast.literal_eval(node.value)
    for node in tree.body
    if isinstance(node, ast.Assign) and isinstance(node.targets[0], ast.Name)
    and node.targets[0].id in {"GET_SCRIPT_INFO_PY", "RUN_ENTRY_POINT_PY"}
}
assert request["lookup"].rstrip() == constants["GET_SCRIPT_INFO_PY"].rstrip()
assert request["entry_point"].rstrip() == constants["RUN_ENTRY_POINT_PY"].rstrip()

names = {"ProbeError", "EntryPoint", "ScriptInfo", "_run_probe", "render_script_not_found"}
context = {
    "__name__": "__main__", "Path": Path, "dataclass": dataclasses.dataclass,
    "json": json, "logger": SimpleNamespace(debug=lambda *args: None),
    "get_environment_for_python": lambda python: {},
}
future = ast.ImportFrom(module="__future__", names=[ast.alias(name="annotations")], level=0)
definitions = [node for node in tree.body if getattr(node, "name", None) in names]
exec(compile(ast.fix_missing_locations(ast.Module(body=[future, *definitions], type_ignores=[])), str(path), "exec"), context)

results = []
for case in request["cases"]:
    if case["mode"] == "message":
        info = context["ScriptInfo"].from_probe(case["document"])
        results.append(context["render_script_not_found"](info))
        continue

    def run(argv, **kwargs):
        assert argv == ["python", "-c", "fixture", "fixture"]
        assert kwargs == dict(capture_output=True, text=True, encoding="utf-8", errors="replace", check=False, timeout=60.0, env={})

        def decode(value):
            return bytes(value).decode("utf-8", "replace").replace("\r\n", "\n").replace("\r", "\n")

        return SimpleNamespace(returncode=case["status"], stdout=decode(case["stdout"]), stderr=decode(case["stderr"]))

    context["subprocess"] = SimpleNamespace(run=run)
    try:
        document = context["_run_probe"](Path("python"), "fixture", ["fixture"])
        results.append({"path": document["path"]})
    except context["ProbeError"] as error:
        results.append({"error": str(error)})

print(json.dumps(results))
