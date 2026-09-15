"""Read Hy-created files through upstream cache getters, without writes."""

import hashlib
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
import hcli.lib.util.cache as cache


def existing_directory(path, *args, **kwargs):
    assert path.is_dir(), f"upstream expected an existing directory: {path}"


with patch.object(cache, "get_default_cache_directory", return_value=Path(sys.argv[2])), \
        patch.object(Path, "mkdir", existing_directory):
    candidates = github.get_candidate_github_repos_cache()
    metadata = github.get_releases_metadata_cache("owner", "repo")
    asset = github.get_release_asset_cache("owner", "repo", "v1", metadata.releases[0].assets[0])
    source = github.get_source_archive_cache("owner", "repo", "source")
print(json.dumps({
    "candidates": candidates,
    "release_name": metadata.releases[0].name,
    "asset": hashlib.sha256(asset).hexdigest(),
    "source": hashlib.sha256(source).hexdigest(),
}))
