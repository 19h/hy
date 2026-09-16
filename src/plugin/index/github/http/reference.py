"""Read-only upstream HTTP failures with transport and response bodies intercepted."""

import http.client
import io
import json
import logging
import os
import sys
import urllib.error
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
        "socket.connect", "socket.bind",
    }:
        raise RuntimeError(f"source oracle attempted mutation/network access: {event}")


sys.addaudithook(require_read_only)
sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
import hcli.lib.ida.plugin.repo.github as github

logging.disable(logging.CRITICAL)


class Socket:
    def __init__(self, data):
        self.data = data

    def makefile(self, *args):
        return io.BytesIO(self.data)


class Body(io.BytesIO):
    def __init__(self, case):
        super().__init__(bytes(case["body"]))
        self.broken = case["broken"]
        self.was_read = False

    def read(self, *args):
        self.was_read = True
        if self.broken:
            raise http.client.IncompleteRead(b"partial", 7)
        return super().read(*args)


def observe(case):
    wire = (
        f"HTTP/1.1 {case['status']} ".encode() + bytes(case["reason"])
        + b"\r\nContent-Length: 0\r\n\r\n"
    )
    response = http.client.HTTPResponse(Socket(wire))
    response.begin()
    body = Body(case)
    failure = urllib.error.HTTPError(
        "https://fixture.test/response", case["status"], response.reason, response.headers, body
    )
    try:
        with patch.object(github, "_urlopen_with_retry", side_effect=failure):
            if case["consumer"] == "graphql":
                github.GitHubGraphQLClient("fixture").query("query Fixture")
            elif case["consumer"] == "search":
                github.find_github_repos_with_plugins("fixture")
            elif case["consumer"] == "asset":
                asset = github.GitHubReleaseAsset(
                    name="plugin.zip", content_type="raw", size=0,
                    download_url="https://fixture.test/asset.zip",
                )
                github.download_release_asset("owner", "repo", "v1", asset)
            else:
                github.download_source_archive("https://fixture.test/source.zip")
    except UnicodeDecodeError as error:
        result = {"kind": "decode", "message": str(error)}
    except http.client.IncompleteRead:
        result = {"kind": "read"}
    except (RuntimeError, urllib.error.HTTPError) as error:
        result = {"kind": "http", "message": str(error)}
    else:
        raise AssertionError("HTTP failure unexpectedly succeeded")
    result["read"] = body.was_read
    return result


print(json.dumps([observe(case) for case in json.load(sys.stdin)]))
