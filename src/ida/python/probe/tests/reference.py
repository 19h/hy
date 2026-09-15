"""Load upstream batch helpers; all source writes are replaced by in-memory adapters."""

import ast
import contextlib
import json
import sys
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

root = Path(sys.argv[1]) / "src/hcli/lib/ida"
request = json.load(sys.stdin)
tree = ast.parse((root / "python/__init__.py").read_text())
payload = next(ast.literal_eval(node.value) for node in tree.body
    if isinstance(node, ast.Assign) and isinstance(node.targets[0], ast.Name)
    and node.targets[0].id == "GET_PYTHON_INFO_PY")
assert request["source"].rstrip() == payload.rstrip()

names = {"_run_ida_batch_script", "_prepare_headless_ida_user_dir"}
tree = ast.parse((root / "__init__.py").read_text())
nodes = [node for node in tree.body if getattr(node, "name", None) in names]
context = {"Path": Path, "json": json, "logger": SimpleNamespace(debug=lambda *args: None), "_log_idat_env": lambda env: None}
future = ast.ImportFrom(module="__future__", names=[ast.alias(name="annotations")], level=0)
exec(compile(ast.fix_missing_locations(ast.Module(body=[future, *nodes], type_ignores=[])), str(root / "__init__.py"), "exec"), context)

results = []
for case in request["cases"]:
    if case["mode"] == "files":
        copied = []
        source = Path(case["source_dir"])
        context["shutil"] = SimpleNamespace(copy2=lambda src, dst: copied.append(str(src.relative_to(source))))
        with patch.object(Path, "mkdir", lambda *args, **kwargs: None):
            context["_prepare_headless_ida_user_dir"](source, Path("/virtual-target"))
        results.append(sorted(copied))
        continue

    text = bytes(case["bytes"]).decode("utf-8", "replace").replace("\r\n", "\n").replace("\r", "\n")
    context["tempfile"] = SimpleNamespace(TemporaryDirectory=lambda: contextlib.nullcontext("/virtual-batch"))

    def run(argv, **kwargs):
        assert argv == ["idat", "-a", "-A", "-c", "-t", "-L/virtual-batch/ida.log", "-S/virtual-batch/idat-script.py"]
        assert kwargs == dict(capture_output=True, text=True, encoding="utf-8", errors="replace", check=False, env={})
        return SimpleNamespace(returncode=17, stdout="__hcli__:{\"stdout\":true}", stderr="ignored")

    context["subprocess"] = SimpleNamespace(run=run)
    with patch.object(Path, "write_text", lambda *args, **kwargs: None), patch.object(Path, "exists", lambda path: True), patch.object(Path, "read_text", lambda *args, **kwargs: text):
        try:
            value = context["_run_ida_batch_script"](Path("idat"), "fixture", env={})
            results.append({"value": value})
        except json.JSONDecodeError:
            results.append({"json_error": True})
        except RuntimeError as error:
            results.append({"error": str(error)})

print(json.dumps(results))
