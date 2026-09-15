"""Read owned Rust fixtures through upstream cache getters; intercept deletion."""

import json
import logging
import os
import sys
from contextlib import ExitStack
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
    path = Path(case["path"])
    removed = []

    def unlink(target, *args, **kwargs):
        assert target == path
        if target.is_dir():
            raise IsADirectoryError(str(target))
        removed.append(str(target))

    with ExitStack() as stack:
        stack.enter_context(patch.object(Path, "unlink", unlink))
        clock = stack.enter_context(patch.object(github.time, "time", return_value=case["now"]))
        stack.enter_context(patch.object(
            github, "get_candidate_github_repos_cache_path", return_value=path
        ))
        stack.enter_context(patch.object(
            github, "get_release_asset_cache_directory", return_value=path.parent
        ))
        stack.enter_context(patch.object(
            github, "get_source_archive_cache_directory", return_value=path.parent
        ))
        stack.enter_context(patch.object(github, "SOURCE_ARCHIVE_FILENAME", path.name))
        try:
            if case["kind"] == "candidates":
                value = github.get_candidate_github_repos_cache()
                assert value == "fixture"
            elif case["kind"] == "asset":
                asset = github.GitHubReleaseAsset(
                    name=path.name, content_type="raw", size=9, download_url="fixture"
                )
                value = github.get_release_asset_cache("owner", "repo", "v1", asset)
                assert value == b'"fixture"'
            else:
                value = github.get_source_archive_cache("owner", "repo", "commit")
                assert value == b'"fixture"'
            state = "hit"
        except KeyError:
            state = "miss"
        except OSError:
            state = "error"
    results.append({
        "state": state,
        "unlinked": bool(removed),
        "clock_reads": clock.call_count,
    })
print(json.dumps(results))
