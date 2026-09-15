"""Read-only projection of actual catalogue collection and getter call order."""

import json
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

results = []
for case in json.load(sys.stdin):
    entries = []
    metadata = {}
    for entry in case:
        owner, name = entry["name"].split("/")
        raw = entry["metadata"]
        metadata[owner, name] = github.GitHubReleases(
            default_branch=github.GitHubCommit.from_dict(raw["defaultBranchRef"]["target"]),
            releases=[github.GitHubRelease.from_dict(value, owner, name) for value in raw["releases"]["nodes"]],
            tags=[github.GitHubTag.from_dict(value) for value in raw["refs"]["nodes"]],
        )

    def asset(owner, repo, tag, value):
        entries.append({"identity":["asset", f"{owner}/{repo}", tag, value.name], "url":value.download_url})
        return b"fixture"

    def source(owner, repo, commit, url):
        entries.append({"identity":["source", f"{owner}/{repo}", commit], "url":url})
        return b"fixture"

    class Index:
        def index_plugin_archive(self, *args, **kwargs):
            pass

        def get_plugins(self):
            return entries

    instance = object.__new__(github.GithubPluginRepo)
    instance._repos = list(metadata)
    instance.client = object()
    with patch.object(github, "get_releases_metadata", side_effect=lambda _, owner, repo: metadata[owner, repo]), \
            patch.object(github, "get_release_asset", side_effect=asset), \
            patch.object(github, "get_source_archive", side_effect=source), \
            patch.object(github, "PluginArchiveIndex", Index), \
            patch.object(github.rich.progress, "track", side_effect=lambda values, **_: values):
        results.append(github.GithubPluginRepo.get_plugins.__wrapped__(instance))
print(json.dumps(results))
