"""Execute source snapshot formatting with a real Pydantic root-model adapter."""

import json
import sys
from pathlib import Path
from types import FunctionType, SimpleNamespace
from typing import Any

sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
from hcli.lib.ida.plugin.repo.file import JSONFilePluginRepo
from pydantic import RootModel

source = JSONFilePluginRepo.to_json
context = dict(source.__globals__)
context["StaticPluginRepo"] = lambda plugins: RootModel[Any](plugins)
render = FunctionType(source.__code__, context)
results = []
for document in json.load(sys.stdin):
    value = json.loads(document)
    results.append(render(SimpleNamespace(get_plugins=lambda: value)))
print(json.dumps(results))
