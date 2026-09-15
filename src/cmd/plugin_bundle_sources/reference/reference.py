"""Exercise actual bundle preprocessing with local-path and repository adapters."""

import importlib
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
bundle = importlib.import_module("hcli.commands.plugin.bundle")


class NonlocalPath:
    def __init__(self, value):
        pass

    def expanduser(self):
        return self

    def exists(self):
        return False


class Repository:
    def fetch_plugin_from_spec(self, spec, platform=None, host=None):
        return {"spec": spec, "host": host}


bundle.Path = NonlocalPath
results = []
for case in json.load(sys.stdin):
    try:
        results.append(bundle._resolve_plugin_bytes(case["input"], Repository()))
    except bundle.click.BadParameter as error:
        results.append({"error": str(error)})
print(json.dumps(results))
