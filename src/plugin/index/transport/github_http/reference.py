"""Read-only upstream GitHub fetch using real HTTPX redirect handling in memory."""

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
import hcli.lib.ida.plugin.repo.github as github

results = []
for case in json.load(sys.stdin):
    requests = []
    operations = [0]

    def respond(request):
        reply = case["replies"][len(requests)]
        names = ["accept", "accept-encoding", "authorization", "cookie"]
        requests.append({
            "url": str(request.url.copy_with(username=None, password=None)),
            "headers": {name: request.headers.get(name) for name in names},
        })
        return httpx.Response(
            reply["status"],
            headers=[(name.encode(), bytes(value)) for name, value in reply["headers"]],
            stream=httpx.ByteStream(bytes(reply["body"])),
        )

    def get(url, **options):
        operations[0] += 1
        if operations[0] == 2:
            return httpx.Response(200, content=b"archive", request=httpx.Request("GET", url))
        with httpx.Client(transport=httpx.MockTransport(respond)) as client:
            return client.get(url, **options)

    with patch.object(github, "GITHUB_API_URL", case["base"]), patch.object(httpx, "get", side_effect=get):
        try:
            github.fetch_github_release_zip_asset("o", "r")
            outcome = {"kind": "success"}
        except httpx.HTTPStatusError as error:
            outcome = {"kind": "http", "status": error.response.status_code, "url": str(error.request.url)}
        except httpx.TooManyRedirects:
            outcome = {"kind": "limit"}
        except httpx.DecodingError:
            outcome = {"kind": "decode"}
        except ValueError as error:
            assert str(error).startswith("HTTPS request was redirected"), str(error)
            outcome = {"kind": "downgrade"}
    results.append({"requests": requests, "result": outcome})
print(json.dumps(results))
