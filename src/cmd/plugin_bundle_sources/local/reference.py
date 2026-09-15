"""Execute the source's resolution loop with in-memory path/read/status adapters."""

import ast
import hashlib
import json
import sys
from contextlib import nullcontext
from pathlib import Path
from types import SimpleNamespace

path = Path(sys.argv[1]) / "src/hcli/commands/plugin/bundle.py"
create = next(
    node for node in ast.parse(path.read_text()).body
    if getattr(node, "name", None) == "create"
)
loop = next(
    node for node in ast.walk(create)
    if isinstance(node, ast.For) and getattr(node.target, "id", None) == "spec"
)
body = []
for node in loop.body:
    if (
        isinstance(node, ast.Assign)
        and getattr(node.targets[0], "id", None) == "unique_hashes"
    ):
        break
    body.append(node)
assert body and len(body) < len(loop.body)
code = compile(ast.Module(body=body, type_ignores=[]), str(path), "exec")

results = []
for case in json.load(sys.stdin):
    calls = []

    def resolve(spec, repo, platform=None):
        index = len(calls)
        calls.append(platform)
        if index == case["failure"]:
            raise OSError("fixture read failure")
        return "fixture", case["samples"][index % len(case["samples"])].encode()

    context = {
        "spec": "~/fixture.zip",
        "parent_repo": None,
        "target_platforms": case["platforms"],
        "hashlib": hashlib,
        "Path": lambda _: SimpleNamespace(exists=lambda: case["read_once"]),
        "rich": SimpleNamespace(status=SimpleNamespace(Status=lambda *a, **kw: nullcontext())),
        "stderr_console": None,
        "_resolve_plugin_bytes": resolve,
    }
    try:
        exec(code, context)
        hashes = context["hash_by_platform"]
        suffixes = len(set(hashes.values())) > 1
        archives = []
        for key, (name, data) in context["archives_by_hash"].items():
            platforms = sorted(p for p, value in hashes.items() if value == key)
            archives.append({
                "name": name,
                "bytes": data.decode(),
                "platforms": platforms if suffixes else [],
            })
        results.append({"calls": calls, "archives": archives})
    except OSError as error:
        results.append({"calls": calls, "error": str(error)})
print(json.dumps(results))
