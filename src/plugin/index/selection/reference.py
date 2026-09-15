"""Select actual source locations from validated, in-memory snapshot fixtures."""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
from hcli.lib.ida.plugin.repo.file import JSONFilePluginRepo
from hcli.lib.ida.plugin.exceptions import AmbiguousPluginReferenceError

results = []
for case in json.load(sys.stdin):
    repository = JSONFilePluginRepo.from_json(json.dumps(case["snapshot"]))
    try:
        location = repository.find_plugin_from_spec(
            case["name"] + case["spec"], case["platform"], case["ida"], host=case["host"]
        )
        results.append({"url": location.url})
    except (ValueError, KeyError, AmbiguousPluginReferenceError):
        results.append({"error": True})
print(json.dumps(results))
