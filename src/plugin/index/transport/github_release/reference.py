"""Read-only release-selection oracle; HTTP request boundaries are intercepted."""

import json
import os
import sys
from pathlib import Path
from unittest.mock import patch


class ForbiddenMutation(BaseException):
    pass


def require_read_only(event, arguments):
    if event == "open" and arguments[2] & (
        os.O_WRONLY | os.O_RDWR | os.O_CREAT | os.O_TRUNC | os.O_APPEND
    ):
        raise ForbiddenMutation("source oracle attempted a file write")
    if event in {
        "os.mkdir", "os.remove", "os.rename", "os.rmdir", "os.link",
        "os.symlink", "os.truncate", "os.chmod", "os.chown", "os.utime",
    }:
        raise ForbiddenMutation(f"source oracle attempted filesystem mutation: {event}")


sys.addaudithook(require_read_only)
sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
import httpx
from hcli.lib.ida.plugin.repo.github import fetch_github_release_zip_asset

results = []
for case in json.load(sys.stdin):
    requests = []

    def get(url, **options):
        request = httpx.Request("GET", url)
        requests.append((url, options))
        body = bytes(case["bytes"]) if len(requests) == 1 else b"archive"
        return httpx.Response(200, content=body, request=request)

    with patch.object(httpx, "get", side_effect=get):
        try:
            fetch_github_release_zip_asset("Owner", "Repo.git", case["tag"])
            outcome = {"kind": "selected", "url": requests[-1][0]}
        except ValueError as error:
            message = str(error)
            if message.startswith(("No .zip", "Multiple .zip", "Asset ")):
                outcome = {"kind": "policy", "message": message}
            else:
                outcome = {"kind": "invalid"}
        except (TypeError, AttributeError, KeyError, httpx.HTTPError):
            outcome = {"kind": "invalid"}
    assert requests[0][1] == {
        "timeout": 30.0, "headers": {"Accept": "application/vnd.github.v3+json"},
        "follow_redirects": True,
    }
    if len(requests) == 2:
        assert requests[1][1] == {"timeout": 60.0, "follow_redirects": True}
    results.append({"endpoint": requests[0][0], "result": outcome})
print(json.dumps(results))
