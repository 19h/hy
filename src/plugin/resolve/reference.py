"""Read-only differential oracle for the upstream dependency resolver."""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
from hcli.lib.ida.plugin import IDAMetadataDescriptor
from hcli.lib.ida.plugin.repo import Plugin
from hcli.lib.ida.plugin.resolve import Cell, InstalledVersion, Requirement, ResolutionError, resolve


class Repository:
    def __init__(self, snapshot):
        self.plugins = [Plugin.model_validate(plugin) for plugin in snapshot["plugins"]]

    def get_plugins(self):
        return self.plugins


def unknown_python():
    raise ValueError("unavailable")


results = []
for case in json.load(sys.stdin):
    cell = case["cell"]
    python = cell["python_version"]
    if cell["check_python"] and python is None:
        python = unknown_python
    fixed = {
        metadata["name"]: IDAMetadataDescriptor.model_validate(
            {"IDAMetadataDescriptorVersion": 1, "plugin": metadata}
        )
        for metadata in case["fixed"]
    }
    installed = {
        metadata["name"]: InstalledVersion(metadata["version"], metadata["urls"]["repository"])
        for metadata in case["installed"]
    }
    try:
        result = resolve(
            [Requirement.from_spec(root) for root in case["roots"]],
            Repository(case["snapshot"]),
            Cell(cell["platform"], cell["ida_version"], python),
            fixed=fixed,
            installed=installed,
        )
        results.append({
            "selected": {name: location.metadata.plugin.version for name, location in result.selected.items()},
            "skipped": len(result.skipped),
            "warnings": len(result.warnings),
        })
    except ResolutionError:
        results.append({"error": True})
print(json.dumps(results))
