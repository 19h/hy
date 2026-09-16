"""Read-only timestamp/MIME checks and source FileHandler retry boundaries."""

import datetime
import io
import json
import logging
import mimetypes
import os
import struct
import sys
import urllib.error
import urllib.request
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch


class ForbiddenMutation(BaseException):
    pass


def require_read_only(event, arguments):
    write_flags = os.O_WRONLY | os.O_RDWR | os.O_CREAT | os.O_TRUNC | os.O_APPEND
    if event == "open" and arguments[2] & write_flags:
        raise ForbiddenMutation("oracle attempted a file write")
    if event in {
        "os.mkdir", "os.remove", "os.rename", "os.rmdir", "os.link",
        "os.symlink", "os.truncate", "os.chmod", "os.chown", "os.utime",
    }:
        raise ForbiddenMutation(f"oracle attempted mutation: {event}")


def from_bits(bits):
    return struct.unpack(">d", bits.to_bytes(8, "big"))[0]


sys.addaudithook(require_read_only)
sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
import hcli.lib.ida.plugin.repo.github as github

logging.disable(logging.CRITICAL)
mimetypes.init()
request = json.load(sys.stdin)
results = []
for case in request["cases"]:
    operation = request["operation"]
    if operation == "metadata":
        value = os.stat(case).st_mtime
        results.append(int.from_bytes(struct.pack(">d", value), "big"))
    elif operation == "timestamps":
        try:
            value = datetime.datetime.fromtimestamp(from_bits(case), datetime.timezone.utc)
            results.append({"datetime": value.replace(tzinfo=None).isoformat(timespec="microseconds")})
        except ValueError as error:
            results.append({"kind": "value", "message": str(error)})
        except OverflowError as error:
            results.append({"kind": "overflow", "message": str(error)})
        except OSError:
            results.append({"kind": "os"})
    elif operation == "mime":
        try:
            mimetypes.guess_type(json.loads(case))
            results.append(True)
        except ValueError:
            results.append(False)
    else:
        req = urllib.request.Request("file:/fixture")
        req.host = case["host"]
        req.selector = json.loads(case["selector"])
        handler = urllib.request.FileHandler()
        stats = SimpleNamespace(st_mtime=from_bits(case["bits"]), st_size=7)
        waits = []
        with patch.object(urllib.request.os, "stat", return_value=stats), \
                patch("builtins.open", side_effect=lambda *args, **kwargs: io.BytesIO(b"payload")), \
                patch.object(urllib.request, "urlopen", side_effect=handler.open_local_file) as opened, \
                patch.object(github.time, "sleep", side_effect=waits.append):
            try:
                with github._urlopen_with_retry(req) as response:
                    outcome = {"kind": "ok", "bytes": list(response.read())}
            except ValueError as error:
                stage = "timestamp" if str(error).startswith(("year ", "Invalid value NaN")) else "selector"
                outcome = {"kind": "value", "stage": stage}
            except OverflowError:
                outcome = {"kind": "overflow", "stage": "timestamp"}
            except urllib.error.URLError as error:
                stage = "host" if str(error.reason) == "file not on local host" else "timestamp"
                outcome = {"kind": "url", "stage": stage}
        results.append({"outcome": outcome, "attempts": opened.call_count, "waits": waits})
print(json.dumps(results))
