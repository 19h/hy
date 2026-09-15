"""Run the actual source catalogue formatter over validated indexed locations."""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
from hcli.lib.ida.plugin.repo import PluginArchiveIndex, PluginArchiveLocation
from hcli.lib.ida.plugin.reference import normalize_plugin_host

results = []
for records in json.load(sys.stdin):
    index = PluginArchiveIndex()
    for record in records:
        location = PluginArchiveLocation.model_validate(record)
        metadata = location.metadata
        plugin = metadata.plugin
        identity = (plugin.name.lower(), normalize_plugin_host(plugin.host))
        spec = (frozenset(plugin.ida_versions), frozenset(plugin.platforms))
        index.index[identity][plugin.version][spec].append(
            (location.url, location.sha256, metadata)
        )
    try:
        plugins = index.get_plugins()
    except TypeError:
        results.append({"error": True})
        continue
    results.append([
        {
            "name": plugin.name,
            "host": plugin.host,
            "versions": [
                [version, [[loc.url, loc.sha256, loc.metadata.plugin.name] for loc in locations]]
                for version, locations in plugin.versions.items()
            ],
        }
        for plugin in plugins
    ])
print(json.dumps(results))
