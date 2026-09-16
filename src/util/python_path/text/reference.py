"""Read-only POSIX path spelling and filesystem encoding oracle."""

import json
import os
import sys
from pathlib import Path, PurePosixPath


def deny_mutation(event, arguments):
    if event == "open" and arguments[2] & (
        os.O_WRONLY | os.O_RDWR | os.O_CREAT | os.O_TRUNC | os.O_APPEND
    ):
        raise RuntimeError("oracle attempted a file write")
    if event in {"os.mkdir", "os.remove", "os.rename", "os.rmdir", "os.link", "os.symlink", "os.truncate", "os.chmod", "os.chown", "os.utime"}:
        raise RuntimeError(f"oracle attempted mutation: {event}")


sys.addaudithook(deny_mutation)
results = []
for document in json.load(sys.stdin):
    name = json.loads(document)
    try:
        results.append(list(os.fsencode(str(PurePosixPath("/fixture/cache") / name))))
    except UnicodeEncodeError:
        results.append(None)
probes = []
for point in (0xdcff, 0xd800):
    try:
        probes.append({"exists": (Path(sys.argv[1]) / f"{chr(point)}.zip").exists()})
    except OSError as error:
        probes.append({"errno": error.errno})
print(json.dumps({"paths": results, "probes": probes}))
