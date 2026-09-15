"""Validate timestamp JSON using the pinned source manifest and locked Pydantic."""

import ast
import json
import sys
from datetime import datetime
from pathlib import Path
from typing import Literal

import pydantic
import pydantic_core
from pydantic import BaseModel, ConfigDict, Field, ValidationError, field_validator

assert pydantic.__version__ == "2.12.5", pydantic.__version__
assert pydantic_core.__version__ == "2.41.5", pydantic_core.__version__
path = Path(sys.argv[1]) / "src/hcli/lib/ida/plugin/repo/bundle.py"
context = dict(globals())
for node in ast.parse(path.read_text()).body:
    if isinstance(node, ast.ClassDef) and node.name in {
        "PluginBundleCreatedBy", "PluginBundleTargetPlatformTag", "PluginBundleManifest",
    }:
        exec(compile(ast.Module(body=[node], type_ignores=[]), str(path), "exec"), context)

results = []
for raw in json.load(sys.stdin):
    document = ('{"version":1,"kind":"hcli-plugin-bundle","builtAt":' + raw
                + ',"createdBy":{"tool":"hcli","version":"0.24.0"},"targetPlatformTags":[]}')
    try:
        manifest = context["PluginBundleManifest"].model_validate_json(document)
        results.append({"value": manifest.built_at.isoformat()})
    except ValidationError as error:
        failure = error.errors(include_url=False)[0]
        assert failure["loc"] == ("builtAt",), failure
        results.append({"error": {"type": failure["type"], "message": failure["msg"]}})
print(json.dumps(results))
