"""Compare installation-relevant ZIP contents without modifying source files."""

import hashlib
import io
import json
import logging
import os
import sys
import zipfile
from pathlib import Path


class ForbiddenMutation(BaseException):
    """Escape the expected packing-error handler if the oracle tries to write."""


def require_read_only(event, arguments):
    if event == "open" and arguments[2] & (
        os.O_WRONLY | os.O_RDWR | os.O_CREAT | os.O_TRUNC | os.O_APPEND
    ):
        raise ForbiddenMutation("source oracle attempted to open a file for writing")
    if event in {
        "os.mkdir", "os.remove", "os.rename", "os.rmdir", "os.link", "os.symlink",
        "os.truncate", "os.chmod", "os.chown", "os.utime",
    }:
        raise ForbiddenMutation(f"source oracle attempted filesystem mutation: {event}")


sys.addaudithook(require_read_only)
sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
from hcli.lib.ida.plugin.install import pack_plugin_directory_to_zip

logging.disable(logging.CRITICAL)
result = []
for directory in json.load(sys.stdin):
    try:
        packed = pack_plugin_directory_to_zip(Path(directory))
    except Exception:
        result.append(None)
        continue
    with zipfile.ZipFile(io.BytesIO(packed)) as archive:
        result.append([
            {
                "name": member.filename,
                "size": member.file_size,
                "sha256": hashlib.sha256(archive.read(member)).hexdigest(),
                "date": member.date_time,
                "permissions": (member.external_attr >> 16) & 0o777,
                "compression": member.compress_type,
            }
            for member in archive.infolist()
        ])
print(json.dumps(result))
