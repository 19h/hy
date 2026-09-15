"""Read-only CPython ZIP oracle; every archive arrives through standard input."""

import base64
import io
import json
import sys
import warnings
import zipfile

warnings.simplefilter("ignore", UserWarning)


def category(error):
    if isinstance(error, (zipfile.BadZipFile, KeyError)):
        return "badzip"
    if isinstance(error, (NotImplementedError, RuntimeError)):
        return "unsupported"
    if isinstance(error, UnicodeDecodeError):
        return "unicode"
    return "other"


def observe(encoded):
    try:
        archive = zipfile.ZipFile(io.BytesIO(base64.b64decode(encoded)))
    except Exception as error:
        return {"error": category(error)}
    with archive:
        reads = []
        for name in archive.namelist():
            try:
                reads.append({"bytes": base64.b64encode(archive.read(name)).decode("ascii")})
            except Exception as error:
                reads.append({"error": category(error)})
        return {"names": archive.namelist(), "reads": reads}


print(json.dumps([observe(case) for case in json.load(sys.stdin)]))
