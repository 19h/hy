"""Read-only source urllib file handler, native paths and acquisition retries."""

import json
import logging
import nturl2path
import os
import sys
import urllib.error
import urllib.request
from pathlib import Path
from unittest.mock import patch


class ForbiddenMutation(BaseException):
    pass


def require_read_only(event, arguments):
    write_flags = os.O_WRONLY | os.O_RDWR | os.O_CREAT | os.O_TRUNC | os.O_APPEND
    if event == "open" and arguments[2] & write_flags:
        raise ForbiddenMutation("source oracle attempted a file write")
    if event in {
        "os.mkdir", "os.remove", "os.rename", "os.rmdir", "os.link",
        "os.symlink", "os.truncate", "os.chmod", "os.chown", "os.utime",
    }:
        raise ForbiddenMutation(f"source oracle attempted mutation: {event}")


sys.addaudithook(require_read_only)
sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
import hcli.lib.ida.plugin.repo.github as github

logging.disable(logging.CRITICAL)
case = json.load(sys.stdin)
results = []
for document in case["documents"]:
    url = json.loads(document)
    if case["operation"] == "read":
        waits = []
        original_open = urllib.request.urlopen
        with patch.object(github.time, "sleep", side_effect=waits.append), \
                patch.object(urllib.request, "urlopen", wraps=original_open) as opened:
            reason = None
            try:
                result = {"kind": "ok", "bytes": list(github.download_source_archive(url))}
            except urllib.error.URLError as error:
                result = {"kind": "url", "bytes": None}
                if isinstance(error.reason, OSError):
                    reason = "system"
                elif "supported only on localhost" in str(error.reason):
                    reason = "remote"
                elif "unknown url type" in str(error.reason):
                    reason = "unknown"
                elif "file not on local host" in str(error.reason):
                    reason = "locality"
                else:
                    raise AssertionError(error.reason)
            except ValueError:
                result = {"kind": "value", "bytes": None}
            except Exception:
                result = {"kind": "other", "bytes": None}
        result["waits"] = waits
        result["attempts"] = opened.call_count
        result["reason"] = reason
        results.append(result)
        continue
    try:
        request = urllib.request.Request(url)
        if request.type != "file":
            results.append(None)
            continue
        selector = request.selector
        try:
            posix = list(os.fsencode(urllib.request.url2pathname(selector)))
        except Exception:
            posix = None
        try:
            windows = list(map(ord, nturl2path.url2pathname(selector)))
        except Exception:
            windows = None
        remote = bool(
            selector[:2] == "//" and selector[2:3] != "/"
            and request.host and request.host != "localhost"
        )
        results.append({
            "host": list(map(ord, request.host or "")),
            "selector": list(map(ord, selector)),
            "posix": posix,
            "windows": windows,
            "remote_double_slash": remote,
        })
    except Exception:
        results.append({"error": True})
print(json.dumps(results))
