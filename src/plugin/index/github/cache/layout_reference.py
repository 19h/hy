"""Capture upstream cache paths and text without mutating the filesystem."""

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
import hcli.lib.util.cache as cache

logging.disable(logging.CRITICAL)
root = Path(sys.argv[2])
results = []
for case in json.load(sys.stdin):
    writes = []

    def write_text(path, text, *args, **kwargs):
        assert kwargs.get("encoding") == "utf-8"
        writes.append({"path": str(path), "text": text})
        return len(text)

    with patch.object(cache, "get_default_cache_directory", return_value=root), \
            patch.object(Path, "mkdir"), patch.object(Path, "write_text", write_text):
        try:
            if case["kind"] == "path":
                directory = cache.get_cache_directory(*case["parts"])
                result = {"directory": str(directory), "path": str(directory / case["filename"])}
            elif case["kind"] == "candidates":
                github.set_candidate_github_repos_cache(case["value"])
                result, = writes
            else:
                model = github.GitHubReleases.model_validate(case["value"])
                github.set_releases_metadata_cache("owner", "repo", model)
                result, = writes
        except ValueError:
            result = {"error": True}
        results.append(result)
print(json.dumps(results))
