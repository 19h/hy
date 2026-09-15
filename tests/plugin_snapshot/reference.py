"""Validate Rust-owned snapshots and select with the actual upstream repository."""

import json
import importlib
import sys
from pathlib import Path

sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
from hcli.lib.ida.plugin.repo.file import JSONFilePluginRepo
from pydantic import ValidationError
from click.testing import CliRunner

try:
    repository = JSONFilePluginRepo.from_file(Path(sys.argv[2]))
    result = {"valid": True}
    if sys.argv[3] == "render":
        command = importlib.import_module("hcli.commands.plugin.repo").snapshot
        output = CliRunner().invoke(command, obj={"plugin_repo": repository})
        result = {"status": output.exit_code, "stdout": output.stdout}
    elif sys.argv[3] == "select":
        try:
            result["selected"] = repository.find_plugin_from_spec("example==1", "linux-x86_64").url
        except ValueError:
            result["selection_error"] = True
except ValidationError:
    result = {"valid": False}
print(json.dumps(result))
