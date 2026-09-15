"""Read-only projections of HTTPX 0.28.1 automatic redirect construction."""

import json
import os
import sys


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
import httpx

assert httpx.__version__ == "0.28.1"
results = []
with httpx.Client() as client:
    for case in json.load(sys.stdin):
        request = httpx.Request("GET", case["base"])
        response = httpx.Response(
            case["status"], headers=[("location", value) for value in case["locations"]]
        )
        try:
            target = client._redirect_url(request, response) if response.has_redirect_location else None
            # An empty HTTPX URL path is sent as `/`. Rust's Url always stores
            # that slash, so compare the request target representation here.
            if target is not None and not target._uri_reference.path:
                target = target.copy_with(path="/")
            results.append({"url": str(target) if target is not None else None})
        except (httpx.InvalidURL, httpx.RemoteProtocolError):
            results.append({"error": True})
print(json.dumps(results))
