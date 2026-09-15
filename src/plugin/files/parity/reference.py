"""Read-only source path grammar, pathlib joins and directory validation."""

import json
import logging
import os
import sys
from pathlib import Path, PurePosixPath, PureWindowsPath


def require_read_only(event, arguments):
    if event == "open" and arguments[2] & (
        os.O_WRONLY | os.O_RDWR | os.O_CREAT | os.O_TRUNC | os.O_APPEND
    ):
        raise RuntimeError("source oracle attempted to open a file for writing")
    if event in {
        "os.mkdir", "os.remove", "os.rename", "os.rmdir", "os.link", "os.symlink",
        "os.truncate", "os.chmod", "os.chown", "os.utime",
    }:
        raise RuntimeError(f"source oracle attempted filesystem mutation: {event}")


sys.addaudithook(require_read_only)
sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
from hcli.lib.ida.plugin import (
    get_python_dependencies_from_plugin_directory,
    validate_path,
)
from hcli.lib.ida.plugin.install import (
    get_metadata_from_plugin_directory,
    validate_metadata_in_plugin_directory,
)

logging.disable(logging.CRITICAL)
request = json.load(sys.stdin)
if "paths" in request:
    valid = []
    for value in request["paths"]:
        try:
            validate_path(value, "fixture")
            valid.append(True)
        except Exception:
            valid.append(False)
    result = {"valid": valid}
    for name, cls in [("posix", PurePosixPath), ("windows", PureWindowsPath)]:
        result[name] = [
            [str(cls(base) / cls(relative)) for relative in request["paths"]]
            for base in request["bases"]
        ]
elif "errors" in request:
    result = []
    for value in request["errors"]:
        try:
            result.append(Path(value).exists())
        except OSError:
            result.append("error")
else:
    result = []
    for value in request["directories"]:
        directory = Path(value)
        metadata = get_metadata_from_plugin_directory(directory)
        try:
            validate_metadata_in_plugin_directory(directory)
            valid = True
        except Exception:
            valid = False
        try:
            dependencies = get_python_dependencies_from_plugin_directory(directory, metadata)
        except Exception:
            dependencies = None
        result.append({"valid": valid, "dependencies": dependencies})
print(json.dumps(result))
