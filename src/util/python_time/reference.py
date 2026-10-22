"""Read-only calls to CPython's actual PyTime_AsSecondsDouble C API."""

import _testcapi
import json
import struct
import sys

print(json.dumps([
    struct.unpack("!Q", struct.pack("!d", _testcapi.PyTime_AsSecondsDouble(value)))[0]
    for value in json.load(sys.stdin)
]))
