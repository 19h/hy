"""Read-only CPython redirect handling, with every follow-up request intercepted."""

import io
import json
import os
import sys
import urllib.error
import urllib.request
from email.message import Message


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
    if event in {"socket.connect", "socket.bind"}:
        raise RuntimeError("source oracle attempted network access")


sys.addaudithook(require_read_only)


class Parent:
    def open(self, request, timeout=None):
        request.timeout = timeout
        return request


class Body(io.BytesIO):
    def __init__(self):
        super().__init__(b"redirect body")
        self.was_read = False
        self.was_closed = False

    def read(self, *args):
        self.was_read = True
        return super().read(*args)

    def close(self):
        self.was_closed = True
        super().close()


def observe(case):
    if case["kind"] == "missing_host":
        request = urllib.request.Request(case["base"])
        try:
            urllib.request.HTTPHandler().do_open(None, request)
        except urllib.error.URLError as error:
            return {"no_host": str(error.reason) == "no host given"}
        raise AssertionError("missing host unexpectedly reached transport")
    method = case.get("method", "GET")
    headers = {
        "Authorization": "Bearer fixture",
        "Content-Type": "application/json",
        "Content-Length": "7",
        "Cookie": "session=fixture",
        "X-Fixture": "retained",
    }
    original = urllib.request.Request(case["base"], headers=headers, method=method)
    original.timeout = None
    request = original
    handler = urllib.request.HTTPRedirectHandler()
    handler.parent = Parent()
    steps = case.get("steps", [{"status": 302, "headers": [["location", case.get("location")]]}])
    events = []
    errors = []
    for step in steps:
        if step.get("restart"):
            request = original
        response_headers = Message()
        for name, value in step["headers"]:
            response_headers[name] = bytes(value).decode("latin1")
        body = Body()
        try:
            if step["status"] in {301, 302, 303, 307, 308}:
                target = handler.http_error_302(
                    request, body, step["status"], "Fixture", response_headers
                )
            else:
                target = None
            if target is None:
                event = {"stop": step["status"]}
            else:
                event = {
                    "url": target.full_url,
                    "method": target.get_method(),
                    "headers": {name.lower(): value for name, value in target.headers.items()},
                }
                request = target
        except urllib.error.HTTPError as error:
            # Observe the boundary while the caller still owns the HTTPError;
            # releasing its response wrapper would close the body during GC.
            errors.append(error)
            event = {"stop": step["status"]}
        except ValueError:
            event = {"error": True}
        if case["kind"] == "target":
            return {"url": event["url"]} if "url" in event else event
        event.update(read=body.was_read, close=body.was_closed)
        events.append(event)
        if "url" not in event:
            break
    return events


print(json.dumps([observe(case) for case in json.load(sys.stdin)]))
