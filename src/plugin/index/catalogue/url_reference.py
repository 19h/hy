"""Read-only source index ordering and snapshot serialization with Python URLs."""

import json
import os
import sys
from pathlib import Path


def deny_mutation(event, arguments):
    write_flags = os.O_WRONLY | os.O_RDWR | os.O_CREAT | os.O_TRUNC | os.O_APPEND
    if event == "open" and arguments[2] & write_flags:
        raise RuntimeError("oracle attempted a file write")
    if event in {
        "os.mkdir", "os.remove", "os.rename", "os.rmdir", "os.link",
        "os.symlink", "os.truncate", "os.chmod", "os.chown", "os.utime",
    }:
        raise RuntimeError(f"oracle attempted mutation: {event}")


sys.addaudithook(deny_mutation)
sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
from hcli.lib.ida.plugin.repo import PluginArchiveIndex, PluginArchiveLocation
from hcli.lib.ida.plugin.repo.file import JSONFilePluginRepo
from hcli.lib.ida.plugin.reference import normalize_plugin_host

results = []
for case in json.load(sys.stdin):
    index = PluginArchiveIndex()
    case["records"][1]["url"] = json.loads(case["url"])
    try:
        PluginArchiveLocation.model_validate_json(json.dumps(case["records"][1]))
        json_input = True
    except ValueError:
        json_input = False
    for record in case["records"]:
        location = PluginArchiveLocation.model_validate(record)
        plugin = location.metadata.plugin
        identity = (plugin.name.lower(), normalize_plugin_host(plugin.host))
        spec = (frozenset(plugin.ida_versions), frozenset(plugin.platforms))
        index.index[identity][plugin.version][spec].append(
            (location.url, location.sha256, location.metadata)
        )
    try:
        plugins = index.get_plugins()
    except TypeError:
        results.append({"error": True})
        continue
    try:
        snapshot = JSONFilePluginRepo(plugins).to_json()
    except ValueError:
        snapshot = None
    results.append({
        "name": plugins[0].name,
        "points": [
            [ord(character) for character in location.url]
            for location in plugins[0].versions["1"]
        ],
        "snapshot": snapshot,
        "json_input": json_input,
    })
print(json.dumps(results))
