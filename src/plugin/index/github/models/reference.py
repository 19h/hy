"""Read-only validation and model_dump through the actual upstream models."""

import json
import os
import sys
from pathlib import Path


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

results = []
for case in json.load(sys.stdin):
    value = case["value"]
    try:
        if case["kind"] == "graphql":
            releases = value["releases"]["nodes"]
            tags = value["refs"]["nodes"]
            model = github.GitHubReleases(
                default_branch=github.GitHubCommit.from_dict(value["defaultBranchRef"]["target"]),
                releases=[github.GitHubRelease.from_dict(item, "owner", "repo") for item in releases],
                tags=[github.GitHubTag.from_dict(item) for item in tags],
            )
        else:
            model = github.GitHubReleases.model_validate(value)
        results.append({"value": model.model_dump()})
    except (ValueError, KeyError, TypeError, AttributeError):
        results.append({"error": True})
print(json.dumps(results))
