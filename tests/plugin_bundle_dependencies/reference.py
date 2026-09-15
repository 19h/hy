"""Read Rust-owned archives with the actual upstream bundle descriptor helpers."""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
from hcli.commands.plugin.bundle import _resolve_plugin_bytes
from hcli.lib.ida.plugin import get_metadatas_with_paths_from_plugin_archive

try:
    name, data = _resolve_plugin_bytes(sys.argv[2], None)
    dependencies = []
    for _, metadata in get_metadatas_with_paths_from_plugin_archive(data):
        if isinstance(metadata.plugin.python_dependencies, list):
            dependencies.extend(metadata.plugin.python_dependencies)
    result = {"name": name, "dependencies": dependencies}
except (ValueError, RuntimeError) as error:
    result = {"error": str(error)}
except OSError as error:
    result = {"error_kind": type(error).__name__}
print(json.dumps(result))
