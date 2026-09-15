"""Read the same Rust-owned files through the actual upstream repository classes."""

import json
import hashlib
import os
import sys
import zipfile
from pathlib import Path


class ForbiddenMutation(BaseException):
    pass


def require_read_only(event, arguments):
    if event == "open" and arguments[2] & (
        os.O_WRONLY | os.O_RDWR | os.O_CREAT | os.O_TRUNC | os.O_APPEND
    ):
        raise ForbiddenMutation("source oracle attempted a file write")
    if event in {
        "os.mkdir", "os.remove", "os.rename", "os.rmdir", "os.link", "os.symlink",
        "os.truncate", "os.chmod", "os.chown", "os.utime",
    }:
        raise ForbiddenMutation(f"source oracle attempted filesystem mutation: {event}")


sys.addaudithook(require_read_only)
sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
from hcli.lib.ida.plugin.repo import repo_from_url
from hcli.lib.ida.plugin.repo.fs import FileSystemPluginRepo
from hcli.lib.ida.plugin.repo.bundle import PluginBundleRepo
from hcli.lib.ida.plugin.repo.file import JSONFilePluginRepo

repository = None
try:
    path = Path(sys.argv[2])
    if sys.argv[3] == "file-url":
        repository = repo_from_url(sys.argv[2])
    else:
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
