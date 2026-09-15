"""Read-only CPython JSON decoding with lossless, flat value projections."""

import json
import os
import struct
import sys


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


def project(value):
    pending = [(False, value)]
    result = []
    while pending:
        key, value = pending.pop()
        if key:
            result.append({"key": [ord(ch) for ch in value]})
        elif value is None:
            result.append({"null": True})
        elif isinstance(value, bool):
            result.append({"bool": value})
        elif isinstance(value, int):
            result.append({"int": str(value)})
        elif isinstance(value, float):
            result.append({"float": struct.pack(">d", value).hex()})
        elif isinstance(value, str):
            result.append({"str": [ord(ch) for ch in value]})
        elif isinstance(value, list):
            result.append({"array": len(value)})
            pending.extend((False, child) for child in reversed(value))
        else:
            result.append({"object": len(value)})
            for key, child in reversed(value.items()):
                pending.extend([(False, child), (True, key)])
    return {"values": result}


sys.addaudithook(require_read_only)
results = []
for document in json.load(sys.stdin):
    try:
        value = json.loads(bytes(document))
    except (ValueError, UnicodeError, RecursionError):
        results.append({"error": True})
    else:
        results.append(project(value))
folds = {
    letter: [point for point in range(0x110000) if chr(point).lower() == letter]
    for letter in "zip"
}
print(json.dumps({"results": results, "zip_folds": folds}))
