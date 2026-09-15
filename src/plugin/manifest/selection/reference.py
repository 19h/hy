"""Read-only selection and validation through the pinned HCLI functions."""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
from hcli.lib.ida.plugin import (
    get_metadata_from_plugin_archive,
    get_metadatas_with_paths_from_plugin_archive,
    validate_metadata_in_plugin_archive,
)

results = []
for case in json.load(sys.stdin):
    data = Path(case["path"]).read_bytes()
    try:
        if case["name"] is None:
            items = list(get_metadatas_with_paths_from_plugin_archive(data))
            if len(items) != 1:
                raise ValueError("requires one plugin")
            path, metadata = items[0]
        else:
            path, metadata = get_metadata_from_plugin_archive(data, case["name"])
        try:
            validate_metadata_in_plugin_archive(data, path, metadata)
            valid = True
        except Exception:
            valid = False
        prefix = "" if path.parent == Path(".") else path.parent.as_posix() + "/"
        results.append(
            {
                "success": True,
                "prefix": prefix,
                "metadata": metadata.plugin.model_dump(mode="json"),
                "references": valid,
            }
        )
    except Exception:
        results.append({"success": False})
print(json.dumps(results))
