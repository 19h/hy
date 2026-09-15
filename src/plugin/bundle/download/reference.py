"""Execute pinned bundle/pip helpers with subprocess calls recorded in memory."""

import ast
import dataclasses
import json
import logging
import re
import sys
import tomllib
from pathlib import Path
from types import SimpleNamespace

import packaging
from packaging.tags import mac_platforms

root = Path(sys.argv[1]) / "src/hcli"
lock = tomllib.loads((Path(sys.argv[1]) / "uv.lock").read_text())
locked_packaging = next(
    package["version"] for package in lock["package"] if package["name"] == "packaging"
)
assert packaging.__version__ == locked_packaging, (
    f"Source wheel tags require packaging {locked_packaging}; oracle has {packaging.__version__}"
)
context = {
    "__name__": "__main__", "dataclass": dataclasses.dataclass, "Path": Path,
    "mac_platforms": mac_platforms, "re": re, "logger": logging.getLogger("oracle"),
}
future = ast.ImportFrom(module="__future__", names=[ast.alias(name="annotations")], level=0)


def load(relative, names):
    path = root / relative
    for node in ast.parse(path.read_text()).body:
        name = getattr(node, "name", None)
        if isinstance(node, ast.Assign):
            name = getattr(node.targets[0], "id", None)
        elif isinstance(node, ast.AnnAssign):
            name = getattr(node.target, "id", None)
        if name in names:
            module = ast.fix_missing_locations(ast.Module(body=[future, node], type_ignores=[]))
            exec(compile(module, str(path), "exec"), context)


load("lib/ida/python/__init__.py", {"PipOptions"})
load("lib/ida/plugin/bundle.py", {
    "_PLATFORM_CONFIG", "_manylinux_tags", "_mac_platform_tags",
    "_build_pip_platform_tags", "PipTarget",
})
load("commands/plugin/bundle.py", {"_download_wheelhouse"})
context["find_current_python_executable"] = lambda: Path("fixture-python")
results = []
for case in json.load(sys.stdin):
    calls = []

    def run(argv, **kwargs):
        assert kwargs == {"capture_output": True, "check": False}
        calls.append(argv)
        return SimpleNamespace(
            returncode=1 if case["mode"] == "error" else 0,
            stdout=bytes(case.get("stdout", [])), stderr=bytes(case.get("stderr", [])),
        )

    context["subprocess"] = SimpleNamespace(run=run)
    values = dict(case.get("options", {}))
    values["offline"] = values.pop("no_index", False)
    values.pop("skip_environment_check", None)
    options = context["PipOptions"](**values)
    target = context["PipTarget"](case["platform"], case["version"])
    try:
        context["_download_wheelhouse"](
            case.get("dependencies", []), target, Path("wheelhouse"), options,
        )
        assert case["mode"] == "plan"
        results.append(calls[0])
    except RuntimeError as error:
        assert case["mode"] == "error"
        results.append(str(error))
print(json.dumps(results))
