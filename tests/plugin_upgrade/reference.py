"""Read the selected archive through the source installation acquisition path."""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
from hcli.lib.ida.plugin import get_metadata_from_plugin_archive
from hcli.lib.ida.plugin.repo.file import JSONFilePluginRepo

repository = JSONFilePluginRepo.from_file(Path(sys.argv[2]))
name, archive = repository.fetch_compatible_plugin_from_spec(
    "example", sys.argv[3], "9.4"
)
try:
    _, metadata = get_metadata_from_plugin_archive(archive, name)
except ValueError as error:
    assert str(error) == f"plugin '{name}' not found in zip archive"
    result = {"missing": True}
else:
    result = {
        "name": metadata.plugin.name,
        "version": metadata.plugin.version,
        "host": metadata.plugin.host,
    }
print(json.dumps(result))
