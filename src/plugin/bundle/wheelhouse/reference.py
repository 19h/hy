"""Execute HCLI's extraction method with an in-memory destination; no file writes."""

import hashlib
import io
import json
import sys
from pathlib import Path
from types import SimpleNamespace

sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
from hcli.lib.ida.plugin.repo.bundle import PluginBundleRepo
import zipfile


class Sink(io.BytesIO):
    def __init__(self, files, name):
        super().__init__()
        self.files = files
        self.name = name
        files[name] = b""

    def close(self):
        if not self.closed:
            self.files[self.name] = self.getvalue()
        super().close()


class File:
    def __init__(self, files, name):
        self.files = files
        self.name = name

    def open(self, mode):
        assert mode == "wb"
        if not self.name:
            raise IsADirectoryError("destination is a directory")
        return Sink(self.files, self.name)


class Destination:
    def __init__(self):
        self.created = False
        self.files = {}

    def mkdir(self, **kwargs):
        self.created = True

    def __truediv__(self, name):
        return File(self.files, name)


results = []
for case in json.load(sys.stdin):
    destination = Destination()
    success = True
    try:
        with zipfile.ZipFile(io.BytesIO(Path(case["path"]).read_bytes())) as archive:
            repo = SimpleNamespace(_zf=archive)
            target = SimpleNamespace(wheelhouse=case["prefix"])
            PluginBundleRepo.extract_wheelhouse(repo, target, destination)
    except Exception:
        success = False
    results.append({"success": success, "created": destination.created, "files": {
        name: {"size": len(data), "sha256": hashlib.sha256(data).hexdigest()}
        for name, data in destination.files.items()
    }})
print(json.dumps(results))
