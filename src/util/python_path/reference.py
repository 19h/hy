"""Compare CPython path expansion and the source find-links callback in memory."""

import ast
import inspect
import json
import ntpath
import os
import pathlib
import sys
import textwrap
from types import SimpleNamespace

def find_links_expression(source):
    callback = next(
        node for node in ast.parse(source.read_text()).body
        if getattr(node, "name", None) == "plugin"
    )
    for node in callback.body:
        if not isinstance(node, ast.Assign):
            continue
        if any(getattr(target, "id", None) == "pip_options" for target in node.targets):
            links = next(value.value for value in node.value.keywords if value.arg == "find_links")
            return compile(ast.Expression(links), str(source), "eval")
    raise AssertionError("Source plugin callback no longer assigns pip_options")


source = pathlib.Path(sys.argv[1]) / "src/hcli/commands/plugin/__init__.py"
expression = find_links_expression(source)
path_context = {}
method = textwrap.dedent(inspect.getsource(pathlib.Path.expanduser))
exec(compile(method, "<CPython Path.expanduser>", "exec"), path_context)


class PosixPath(pathlib.PurePosixPath):
    expanduser = path_context["expanduser"]


class WindowsPath(pathlib.PureWindowsPath):
    expanduser = path_context["expanduser"]


results = []
for case in json.load(sys.stdin):
    flavor = case.get("flavor", "windows" if case["mode"] == "windows_home" else "posix")
    if case["mode"] == "lexical":
        expand = lambda value: case["home"] if case["home"] is not None else value
    elif case["mode"] == "windows_home":
        ntpath.os = SimpleNamespace(**(vars(os) | {"environ": case["env"]}))
        expand = ntpath.expanduser
    else:
        expand = os.path.expanduser
    path_context["os"] = SimpleNamespace(path=SimpleNamespace(expanduser=expand))
    try:
        path_type = WindowsPath if flavor == "windows" else PosixPath
        if case.get("preserve_urls", True):
            result = eval(expression, {"Path": path_type, "pip_find_links": (case["value"],)})[0]
        else:
            result = path_type(case["value"]).expanduser()
        results.append({"value": str(result)})
    except (RuntimeError, ValueError) as error:
        results.append({"error": str(error)})
print(json.dumps(results))
