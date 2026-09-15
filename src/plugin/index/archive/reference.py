"""Run source archive indexing over Rust-owned bytes and compare serialized catalogues."""

import json
import sys
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
from hcli.lib.ida.plugin.repo import PluginArchiveIndex
from hcli.lib.ida.plugin.repo.file import JSONFilePluginRepo

results = []
for case in json.load(sys.stdin):
    index = PluginArchiveIndex()
    try:
        index.index_plugin_archive(Path(case["path"]).read_bytes(), "fixture:archive", expected_host=case["host"])
        snapshot = JSONFilePluginRepo(index.get_plugins()).to_json()
        results.append({"success": True, "snapshot": json.loads(snapshot)})
    except (ValueError, TypeError, zipfile.BadZipFile):
        results.append({"success": False})
print(json.dumps(results))
