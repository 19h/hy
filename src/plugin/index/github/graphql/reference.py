"""Read-only queries and response projections from the actual GitHub client."""

import io
import json
import logging
import os
import sys
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
    }:
        raise RuntimeError(f"source oracle attempted filesystem mutation: {event}")


sys.addaudithook(require_read_only)
sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
import hcli.lib.ida.plugin.repo.github as github

logging.disable(logging.CRITICAL)
results = []
for case in json.load(sys.stdin):
    requests = []

    def respond(request):
        body = json.loads(request.data)
        body["query"] = " ".join(body["query"].split())
        requests.append(body)
        return io.BytesIO(json.dumps(case["response"]).encode())

    with patch.object(github, "_urlopen_with_retry", side_effect=respond):
        try:
            values = github.GitHubGraphQLClient("fixture").get_many_releases(
                [tuple(name.split("/")) for name in case["repositories"]]
            )
            outcome = {"repositories": ["/".join(name) for name in values]}
        except RuntimeError as error:
            if not str(error).startswith("GraphQL errors:"):
                raise
            outcome = {"fatal": str(error)}
        except (ValueError, TypeError, AttributeError, KeyError):
            outcome = {"error": True}
    assert len(requests) <= 1
    results.append({"request": requests[0] if requests else None, "outcome": outcome})
print(json.dumps(results))
