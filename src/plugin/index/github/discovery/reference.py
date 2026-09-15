"""Read-only source discovery with list-file and network operations intercepted."""

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
import hcli.commands.plugin as command
import hcli.lib.ida.plugin.repo.github as github
from hcli.lib.util.cache import validate_path_component

logging.disable(logging.CRITICAL)
results = []
for case in json.load(sys.stdin):
    urls = []
    publication = []
    try:
        if case["kind"] == "list":
            with patch.object(Path, "exists", return_value=True), patch.object(
                Path, "read_text", return_value=case["text"]
            ):
                value = command.read_repos_file(Path("fixture"))
        elif case["kind"] == "select":
            instance = object.__new__(github.GithubPluginRepo)
            instance.extra_repos = set(case["extra"])
            instance.ignored_repos = set(case["ignored"])
            with patch.object(github, "get_candidate_github_repos_cache", return_value=case["candidates"]):
                value = ["/".join(parts) for parts in instance._get_repos()]
        elif case["kind"] == "cache":
            instance = object.__new__(github.GithubPluginRepo)
            instance.extra_repos = set()
            instance.ignored_repos = set(case["ignored"])
            with patch.object(github, "get_candidate_github_repos_cache", return_value=case["root"]):
                value = ["/".join(parts) for parts in instance._get_repos()]
        elif case["kind"] == "component":
            try:
                for part in github.parse_repository(case["name"]):
                    validate_path_component(part)
                valid = True
            except ValueError:
                valid = False
            results.append({"valid": valid})
            continue
        else:
            def respond(request):
                urls.append(request.full_url)
                body = case["response"] if request.full_url.endswith("page=1") else {}
                assert request.get_header("Accept") == "application/vnd.github.v3+json"
                assert request.get_header("User-agent") == "ida-hcli"
                return io.BytesIO(json.dumps(body).encode())

            with patch.object(github, "_urlopen_with_retry", side_effect=respond):
                if case["kind"] == "pipeline":
                    instance = object.__new__(github.GithubPluginRepo)
                    instance.token = "fixture"
                    instance.extra_repos = set()
                    instance.ignored_repos = set()
                    with patch.object(
                        github, "get_candidate_github_repos_cache", side_effect=KeyError
                    ), patch.object(
                        github, "set_candidate_github_repos_cache", side_effect=publication.append
                    ):
                        value = ["/".join(parts) for parts in instance._get_repos()]
                else:
                    value = github.find_github_repos_with_plugins("fixture")
        result = {"value": value}
    except (ValueError, TypeError, AttributeError, KeyError):
        result = {"error": True}
    if case["kind"] == "search":
        result["urls"] = urls
    elif case["kind"] in {"cache", "pipeline"}:
        result["urls"] = urls
        result["published"] = publication[0] if publication else None
    results.append(result)
print(json.dumps(results))
