"""Read Rust-owned archives with the actual upstream bundle descriptor helpers."""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
from hcli.commands.plugin.bundle import _read_local_plugin, _get_local_metadata, _collect_all_python_deps

try:
    name, data = _read_local_plugin(sys.argv[2])
    _get_local_metadata(data)
    dependencies = _collect_all_python_deps(data)
    result = {"name": name, "dependencies": dependencies}
except (ValueError, RuntimeError) as error:
    result = {"error": str(error)}
except OSError as error:
    result = {"error_kind": type(error).__name__}
print(json.dumps(result))
