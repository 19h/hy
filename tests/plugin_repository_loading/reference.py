"""Read the same Rust-owned files through the actual upstream repository classes."""

import json
import hashlib
import sys
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
from hcli.lib.ida.plugin.repo.fs import FileSystemPluginRepo
from hcli.lib.ida.plugin.repo.bundle import PluginBundleRepo
from hcli.lib.ida.plugin.repo.file import JSONFilePluginRepo

repository = None
try:
    path = Path(sys.argv[2])
    repository = PluginBundleRepo(path) if sys.argv[3].startswith("bundle") else FileSystemPluginRepo(path)
    if sys.argv[3] == "bundle-fetch":
        location = repository.find_plugin_from_spec("example==1", "linux-x86_64")
        name, archive = repository._fetch_and_verify(location)
        result = {"name": name, "sha256": hashlib.sha256(archive).hexdigest()}
    else:
        snapshot = JSONFilePluginRepo.from_repo(repository).to_json()
        result = {"success": True, "snapshot": json.loads(snapshot)}
except (OSError, ValueError, KeyError, zipfile.BadZipFile, NotImplementedError, RuntimeError, TypeError):
    result = {"success": False}
finally:
    if isinstance(repository, PluginBundleRepo):
        repository.close()
print(json.dumps(result))
