"""Compare requiresPython grammar and membership with HCLI's locked packaging."""

import json
import sys

from packaging.specifiers import InvalidSpecifier, SpecifierSet
from packaging.version import InvalidVersion, Version

results = []
for case in json.load(sys.stdin):
    valid = False
    matches = False
    try:
        requirement = SpecifierSet(case["requirement"].strip())
        valid = bool(case["requirement"].strip())
    except InvalidSpecifier:
        pass
    if valid:
        try:
            matches = Version(case["version"]) in requirement
        except InvalidVersion:
            pass
    results.append({"valid": valid, "matches": matches})
print(json.dumps(results))
