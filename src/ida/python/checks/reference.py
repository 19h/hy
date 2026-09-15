"""Read-only source probe with CPython's real subprocess text translator."""

import ast
import json
import logging
import subprocess
import sys
from pathlib import Path
from types import SimpleNamespace

path = Path(sys.argv[1]) / "src/hcli/lib/venv.py"
context = {"Path": Path, "logger": logging.getLogger("oracle")}
for node in ast.parse(path.read_text()).body:
    name = getattr(node, "name", None)
    if isinstance(node, ast.Assign):
        name = getattr(node.targets[0], "id", None)
    if name in {"PRINT_VERSION_PY", "probe_python_version"}:
        exec(compile(ast.Module(body=[node], type_ignores=[]), str(path), "exec"), context)

pip_path = Path(sys.argv[1]) / "src/hcli/lib/ida/python/__init__.py"
pip_node = next(node for node in ast.parse(pip_path.read_text()).body
                if getattr(node, "name", None) == "has_pip")
exec(compile(ast.Module(body=[pip_node], type_ignores=[]), str(pip_path), "exec"), context)


def translate(value):
    return subprocess.Popen._translate_newlines(None, value, "utf-8", "strict")


results = []
for case in json.load(sys.stdin):
    def run(argv, **kwargs):
        if case["mode"] == "pip":
            assert argv == ["fixture-python", "-c", "import pip"]
            assert kwargs == {"capture_output": True, "timeout": 10.0, "check": False}
            return subprocess.CompletedProcess(
                argv, case["status"], bytes(case["stdout"]), bytes(case["stderr"]),
            )
        assert argv == ["fixture-python", "-c", context["PRINT_VERSION_PY"]]
        assert kwargs == {"capture_output": True, "text": True, "check": True, "timeout": 10.0}
        stdout = translate(bytes(case["stdout"]))
        stderr = translate(bytes(case["stderr"]))
        if not case["success"]:
            raise subprocess.CalledProcessError(7, argv, stdout, stderr)
        return subprocess.CompletedProcess(argv, 0, stdout, stderr)

    context["subprocess"] = SimpleNamespace(
        run=run, SubprocessError=subprocess.SubprocessError, TimeoutExpired=subprocess.TimeoutExpired,
    )
    try:
        if case["mode"] == "pip":
            value = context["has_pip"](Path("fixture-python"))
        elif case["mode"] == "text":
            value = translate(bytes(case["bytes"]))
        else:
            value = context["probe_python_version"](Path("fixture-python"))
        results.append({"value": value})
    except UnicodeDecodeError as error:
        results.append({"error": str(error)})
print(json.dumps(results))
