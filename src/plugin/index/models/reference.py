"""Validate snapshot envelopes with upstream's actual Pydantic repository models."""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
from hcli.lib.ida.plugin.repo.file import StaticPluginRepo
from pydantic import ValidationError

results = []
for case in json.load(sys.stdin):
    try:
        snapshot = StaticPluginRepo.model_validate_json(case["document"])
        plugins = []
        for plugin in snapshot.plugins:
            versions = []
            for version, locations in plugin.versions.items():
                records = []
                for location in locations:
                    descriptor = location.metadata.model_dump(mode="json")
                    records.append({
                        "url": location.url,
                        "sha256": location.sha256,
                        "descriptor_version": descriptor["IDAMetadataDescriptorVersion"],
                        "has_schema": "$schema" in descriptor,
                    })
                versions.append({"version": version, "locations": records})
            plugins.append({"name": plugin.name, "host": plugin.host, "versions": versions})
        results.append({"version": snapshot.version, "plugins": plugins})
    except ValidationError:
        results.append({"invalid": True})
print(json.dumps(results))
