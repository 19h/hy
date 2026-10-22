"""Read-only clock injection into upstream reactive and proactive wait functions."""

import _testcapi
import json
import logging
import os
import sys
import urllib.error
from email.message import Message
from fractions import Fraction
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch


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
sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
import hcli.lib.ida.plugin.repo.github as github

logging.disable(logging.CRITICAL)
results = []
for case in json.load(sys.stdin):
    headers = Message()
    for name, value in case["headers"]:
        headers[name] = value

    def now():
        if case["fail_clock"]:
            raise RuntimeError("clock")
        return _testcapi.PyTime_AsSecondsDouble(case["now_ns"])

    with patch.object(github.time, "time", side_effect=now) as clock, \
            patch.object(github.time, "sleep") as sleep:
        try:
            if case["reactive"]:
                error = urllib.error.HTTPError("https://fixture", 429, "fixture", headers, None)
                state = SimpleNamespace(
                    attempt_number=1,
                    outcome=SimpleNamespace(failed=True, exception=lambda: error),
                )
                wait = github.WaitGitHubRateLimit()(state)
            else:
                github._check_and_handle_proactive_rate_limit(SimpleNamespace(headers=headers))
                wait = sleep.call_args.args[0] if sleep.called else None
            # Rust Duration represents waits in whole nanoseconds, rounding ties
            # to even. Fraction avoids an extra binary64 multiplication rounding.
            outcome = {"wait_ns": round(Fraction(wait) * 1_000_000_000) if wait is not None else None}
        except ValueError:
            outcome = {"error": "header"}
        except OverflowError:
            outcome = {"error": "overflow"}
        except RuntimeError as error:
            assert str(error) == "clock"
            outcome = {"error": "clock"}
    results.append({"clock_reads": clock.call_count, "outcome": outcome})
print(json.dumps(results))
