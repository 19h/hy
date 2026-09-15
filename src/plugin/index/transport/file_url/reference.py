"""Read-only urllib file URL conversion and archive acquisition oracle."""

import json
import os
import sys
import unicodedata
from pathlib import Path, PurePosixPath, PureWindowsPath
from urllib.parse import urlparse
import nturl2path


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
from hcli.lib.ida.plugin.repo import fetch_plugin_archive
from urllib.request import url2pathname

request = json.load(sys.stdin)
if request["operation"] == "delimiters":
    result = [
        code for code in range(128, 0x110000)
        if any(char in unicodedata.normalize("NFKC", chr(code)) for char in "/?#@:")
    ]
elif request["operation"] == "read":
    result = []
    for value in request["values"]:
        try:
            result.append(list(fetch_plugin_archive(value)))
        except Exception:
            result.append(None)
else:
    result = []
    for value in request["values"]:
        try:
            parsed = urlparse(value)
            if parsed.scheme != "file":
                result.append(None)
                continue
            posix = list(os.fsencode(str(PurePosixPath(url2pathname(parsed.path)))))
            try:
                windows = str(PureWindowsPath(nturl2path.url2pathname(parsed.path)))
            except Exception:
                windows = None
            result.append({"posix": posix, "windows": windows})
        except Exception:
            result.append(None)
print(json.dumps(result))
