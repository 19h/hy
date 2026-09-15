"""Compare native permutations with CPython's actual tuple-of-frozensets sort."""

import json
import sys

def key(pair):
    return tuple(frozenset(bit for bit in range(8) if mask & (1 << bit)) for mask in pair)

results = []
for case in json.load(sys.stdin):
    keys = [key(pair) for pair in case]
    results.append(sorted(range(len(keys)), key=keys.__getitem__))
print(json.dumps(results))
