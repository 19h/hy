"""Evaluate upstream acquisition branches without downloading or installing."""

import ast
import json
import os
import sys
from pathlib import Path


class ForbiddenMutation(BaseException):
    pass


def require_read_only(event, arguments):
    if event == "open" and arguments[2] & (
        os.O_WRONLY | os.O_RDWR | os.O_CREAT | os.O_TRUNC | os.O_APPEND
    ):
        raise ForbiddenMutation("source oracle attempted a file write")
    if event in {
        "os.mkdir", "os.remove", "os.rename", "os.rmdir", "os.link", "os.symlink",
        "os.truncate", "os.chmod", "os.chown", "os.utime",
    }:
        raise ForbiddenMutation(f"source oracle attempted filesystem mutation: {event}")


sys.addaudithook(require_read_only)
source = Path(sys.argv[1])
sys.path.insert(0, str(source / "src"))
import click
from hcli.lib.ida.plugin.reference import is_github_direct_install_url

module = ast.parse((source / "src/hcli/commands/plugin/install.py").read_text())
function = next(node for node in module.body if isinstance(node, ast.FunctionDef) and node.name == "install_plugin")
branches = next(
    node for node in ast.walk(function)
    if isinstance(node, ast.If) and isinstance(node.test, ast.Name) and node.test.id == "editable"
)

# Keep the source conditions and editable path checks. Replace acquisition bodies
# with observations; no source downloader, packer, pip or publication is invoked.
branch = branches
for kind in ["directory", "directory", "archive", "download", "github", "download"]:
    if isinstance(branch.test, ast.Name) and branch.test.id == "editable":
        prefix = []
        for statement in branch.body:
            if isinstance(statement, ast.Try):
                break
            prefix.append(statement)
        branch.body = prefix + ast.parse("return 'directory'").body
    else:
        branch.body = ast.parse(f"return {kind!r}").body
    if len(branch.orelse) == 1 and isinstance(branch.orelse[0], ast.If):
        branch = branch.orelse[0]
    else:
        branch.orelse = ast.parse("return 'repository'").body
        break
wrapper = ast.parse("def classify(plugin_spec, editable):\n    pass\n")
wrapper.body[0].body = [branches]
exec(compile(ast.fix_missing_locations(wrapper), "<source acquisition branches>", "exec"))
result = []
for value, editable in json.load(sys.stdin):
    try:
        result.append(classify(value, editable))
    except Exception:
        result.append("error")
print(json.dumps(result))
