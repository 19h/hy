"""Execute the source target grammar and selector with in-memory observations."""

import ast
import dataclasses
import json
import re
import sys
import tomllib
from pathlib import Path

import click
import packaging
from packaging.tags import mac_platforms

source = Path(sys.argv[1])
lock = tomllib.loads((source / "uv.lock").read_text())
assert packaging.__version__ == next(
    package["version"] for package in lock["package"] if package["name"] == "packaging"
)
context = {
    "__name__": "__main__", "dataclass": dataclasses.dataclass, "re": re,
    "mac_platforms": mac_platforms, "click": click,
}
future = ast.ImportFrom(module="__future__", names=[ast.alias(name="annotations")], level=0)


class InjectImports(ast.NodeTransformer):
    def visit_ImportFrom(self, node):
        return ast.copy_location(ast.Pass(), node)


def load(relative, names):
    path = source / "src/hcli" / relative
    for node in ast.parse(path.read_text()).body:
        name = getattr(node, "name", None)
        if isinstance(node, ast.Assign):
            name = getattr(node.targets[0], "id", None)
        elif isinstance(node, ast.AnnAssign):
            name = getattr(node.target, "id", None)
        if name in names:
            if name == "_resolve_targets":
                node = InjectImports().visit(node)
            module = ast.fix_missing_locations(ast.Module(body=[future, node], type_ignores=[]))
            exec(compile(module, str(path), "exec"), context)


load("lib/ida/plugin/bundle.py", {
    "MINIMUM_PYTHON_VERSION", "SUPPORTED_PYTHON_VERSIONS", "PLATFORM_ALIASES",
    "ALL_PLATFORMS", "_PLATFORM_CONFIG", "resolve_platform_alias", "_manylinux_tags",
    "_mac_platform_tags", "_build_pip_platform_tags", "_parse_python_version", "PipTarget",
})
load("commands/plugin/bundle.py", {"_resolve_targets"})


def outcome(operation):
    try:
        return {"value": operation()}
    except (ValueError, click.BadParameter, RuntimeError) as error:
        return {"error": str(error)}


def record(target, tags=False):
    result = {"id": target.id, "platform": target.ida_platform, "version": target.python_version}
    if tags:
        result.update(abis=target.abis, tags=outcome(lambda: target.pip_platform_tags))
    return result


results = []
for case in json.load(sys.stdin):
    calls = []

    def observe(name, key):
        calls.append(name)
        if case[key] is None:
            raise RuntimeError(f"{name} failed")
        return case[key]

    context["find_current_ida_platform"] = lambda: observe("platform", "platform")
    context["detect_current_python_version"] = lambda: observe("python", "version")
    if case["mode"] == "alias":
        result = outcome(lambda: context["resolve_platform_alias"](case["platform"]))
    elif case["mode"] == "parse":
        result = outcome(lambda: record(context["PipTarget"].parse(case["id"]), tags=True))
    elif case["mode"] == "new":
        result = outcome(lambda: record(
            context["_resolve_targets"](("current",), (case["version"],), ())[0], tags=True,
        ))
    else:
        result = outcome(lambda: [record(target) for target in context["_resolve_targets"](
            tuple(case["platforms"]), tuple(case["pythons"]), tuple(case["targets"]),
        )])
        result = {"result": result, "calls": calls}
    results.append(result)
print(json.dumps(results))
