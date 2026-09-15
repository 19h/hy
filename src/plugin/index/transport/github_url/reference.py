"""Read-only direct-install GitHub recognition and parsing oracle."""

import json
import os
import sys
from pathlib import Path


class ForbiddenMutation(BaseException):
    pass


def require_read_only(event, arguments):
    if event == "open" and arguments[2] & (
        os.O_WRONLY | os.O_RDWR | os.O_CREAT | os.O_TRUNC | os.O_APPEND
    ):
        raise ForbiddenMutation("source oracle attempted a file write")
    if event in {
        "os.mkdir", "os.remove", "os.rename", "os.rmdir", "os.link", "os.symlink",
        "os.truncate", "os.chmod", "os.chown", "os.utime",
    }:
        raise ForbiddenMutation(f"source oracle attempted filesystem mutation: {event}")


sys.addaudithook(require_read_only)
sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
from hcli.lib.ida.plugin.reference import is_github_direct_install_url
from hcli.lib.ida.plugin.repo.github import parse_github_url

result = []
for value in json.load(sys.stdin):
    recognized = is_github_direct_install_url(value)
    parsed = None
    if recognized:
        try:
            parsed = parse_github_url(value)
        except Exception:
            pass
    result.append({"recognized": recognized, "parsed": parsed})
print(json.dumps(result))
