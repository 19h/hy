"""Run the upstream resolution loop with read-only repository transport adapters."""

import ast
import hashlib
import importlib
import json
import sys
from contextlib import nullcontext
from pathlib import Path
from types import SimpleNamespace

source = Path(sys.argv[1])
sys.path.insert(0, str(source / "src"))
bundle = importlib.import_module("hcli.commands.plugin.bundle")
repo = importlib.import_module("hcli.lib.ida.plugin.repo")
case = json.load(sys.stdin)
fetches = []
locations = []
for index, location in enumerate(case["locations"]):
    locations.append(repo.PluginArchiveLocation.model_validate({
        "url": f"fixture:{index}",
        "sha256": location["sha256"],
        "metadata": location["metadata"],
    }))


class Repository(repo.BasePluginRepo):
    def get_plugins(self):
        return [repo.Plugin(
            name="example",
            host="https://github.com/example/original",
            versions={"1": locations},
        )]


def fetch(url):
    index = int(url.removeprefix("fixture:"))
    fetches.append(index)
    return Path(case["locations"][index]["path"]).read_bytes()


repo.fetch_plugin_archive = fetch
path = source / "src/hcli/commands/plugin/bundle.py"
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
    if isinstance(node, ast.Assign) and getattr(node.targets[0], "id", None) == "unique_hashes":
        break
    body.append(node)
assert body and len(body) < len(loop.body)
context = {
    "spec": case["spec"], "parent_repo": Repository(),
    "target_platforms": case["platforms"], "hashlib": hashlib, "Path": Path,
    "rich": SimpleNamespace(status=SimpleNamespace(Status=lambda *a, **kw: nullcontext())),
    "stderr_console": None, "_resolve_plugin_bytes": bundle._resolve_plugin_bytes,
}
try:
    exec(compile(ast.Module(body=body, type_ignores=[]), str(path), "exec"), context)
    hashes = context["hash_by_platform"]
    files = []
    dependencies = []
    for checksum, (name, data) in context["archives_by_hash"].items():
        version = bundle.get_version_from_plugin_archive(data, name)
        suffix = ""
        if len(set(hashes.values())) > 1:
            suffix = "-" + "+".join(sorted(p for p, value in hashes.items() if value == checksum))
        files.append({"file": f"plugins/{name}-{version}{suffix}.zip", "sha256": checksum})
        for _, metadata in bundle.get_metadatas_with_paths_from_plugin_archive(data):
            if isinstance(metadata.plugin.python_dependencies, list):
                dependencies.extend(metadata.plugin.python_dependencies)
    result = {
        "fetches": fetches,
        "files": sorted(files, key=lambda item: item["file"]),
        "dependencies": dependencies,
    }
except (ValueError, bundle.click.BadParameter) as error:
    message = str(error)
    if message.startswith("hash mismatch:"):
        message = "hash mismatch"
    result = {"fetches": fetches, "error": message}
print(json.dumps(result))
