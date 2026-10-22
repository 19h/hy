"""Resolve and stage upstream bundle cells using owned archives and no network."""

import hashlib
import importlib
import json
import sys
import tempfile
from pathlib import Path

source = Path(sys.argv[1])
sys.path.insert(0, str(source / "src"))
bundle = importlib.import_module("hcli.commands.plugin.bundle")
repo = importlib.import_module("hcli.lib.ida.plugin.repo")
from hcli.lib.ida.plugin.reference import parse_plugin_reference

case = json.load(sys.stdin)
fetches = []
locations = [repo.PluginArchiveLocation.model_validate({
    "url": f"fixture:{index}", "sha256": location["sha256"], "metadata": location["metadata"],
}) for index, location in enumerate(case["locations"])]


class Repository(repo.BasePluginRepo):
    def get_plugins(self):
        return [repo.Plugin(name="example", host="https://github.com/example/original", versions={"1": locations})]


def fetch(url):
    index = int(url.removeprefix("fixture:"))
    fetches.append(index)
    return Path(case["locations"][index]["path"]).read_bytes()


repo.fetch_plugin_archive = fetch
try:
    if parse_plugin_reference(case["spec"]).repo is not None:
        raise ValueError("repository prefixes cannot be combined with --repo")
    targets = [bundle.PipTarget(platform, "3.12") for platform in ["windows-x86_64", "linux-x86_64", "macos-aarch64"]]
    resolution = bundle._get_cell_closures((case["spec"],), targets, Repository())
    by_name = {}
    dependencies = []
    for target, closure in resolution.closures.items():
        dependencies.extend(bundle._get_python_deps(closure))
        for name, data in closure.items():
            by_name.setdefault(name, {})[target] = data
    with tempfile.TemporaryDirectory() as directory:
        destination = Path(directory)
        for name, by_target in by_name.items():
            bundle._stage_plugin_archives(name, by_target, destination, repo.PluginArchiveIndex())
        files = [{"file": "plugins/" + path.name, "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
                 for path in sorted(destination.iterdir())]
    result = {"fetches": fetches, "files": files, "dependencies": list(dict.fromkeys(dependencies))}
except (ValueError, RuntimeError) as error:
    result = {"fetches": fetches, "error": str(error)}
print(json.dumps(result))
