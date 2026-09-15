"""Compare reference parsing with the actual checked-out source module."""

import json
import sys
from dataclasses import asdict
from pathlib import Path

sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
from hcli.lib.ida.plugin.reference import parse_plugin_reference

results = []
for case in json.load(sys.stdin):
    try:
        results.append(asdict(parse_plugin_reference(case["input"])))
    except ValueError as error:
        results.append({"error": str(error)})
print(json.dumps(results))
