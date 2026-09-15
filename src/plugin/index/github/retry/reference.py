"""Read-only execution of the actual upstream nested Tenacity decorators."""

import io
import json
import logging
import os
import sys
import urllib.error
from email.message import Message
from pathlib import Path
from unittest.mock import patch


def require_read_only(event, arguments):
    if event == "open" and arguments[2] & (
        os.O_WRONLY | os.O_RDWR | os.O_CREAT | os.O_TRUNC | os.O_APPEND
    ):
        raise RuntimeError("source oracle attempted a file write")
    if event in {
        "os.mkdir", "os.remove", "os.rename", "os.rmdir", "os.link",
        "os.symlink", "os.truncate", "os.chmod", "os.chown", "os.utime",
    }:
        raise RuntimeError(f"source oracle attempted filesystem mutation: {event}")


sys.addaudithook(require_read_only)
sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
import hcli.lib.ida.plugin.repo.github as github

logging.disable(logging.CRITICAL)
results = []
for steps in json.load(sys.stdin):
    events = []
    clock = [1700000000.25]
    sequence = iter(steps)

    def wait(seconds):
        events.append(["wait", float(seconds)])
        clock[0] += seconds

    def open_request(request):
        events.append(["request", clock[0]])
        step = next(sequence)
        if step["kind"] == "transient":
            raise urllib.error.URLError("fixture")
        if step["kind"] == "timeout":
            raise TimeoutError("fixture")
        if step["kind"] == "terminal":
            raise RuntimeError("terminal")
        headers = Message()
        for name, value in step["headers"]:
            headers[name] = bytes(value).decode("latin-1")
        status = step["status"]
        if not 200 <= status < 300:
            raise urllib.error.HTTPError(request.full_url, status, "fixture", headers, io.BytesIO())
        response = io.BytesIO()
        response.status = status
        response.headers = headers
        return response

    with patch.object(github.urllib.request, "urlopen", side_effect=open_request), \
            patch.object(github.time, "time", side_effect=lambda: clock[0]), \
            patch.object(github.time, "sleep", side_effect=wait):
        try:
            response = github._urlopen_with_retry(github.urllib.request.Request("https://api.github.com/fixture"))
            outcome = {"status": response.status}
        except urllib.error.HTTPError as error:
            outcome = {"status": error.code}
        except (urllib.error.URLError, TimeoutError):
            outcome = {"error": "transport"}
        except ValueError:
            outcome = {"error": "header"}
        except OverflowError:
            outcome = {"error": "overflow"}
        except RuntimeError as error:
            assert str(error) == "terminal", str(error)
            outcome = {"error": "terminal"}
    results.append({"events": events, "outcome": outcome})
print(json.dumps(results))
