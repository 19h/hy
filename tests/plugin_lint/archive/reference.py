"""Read-only lint reports from the pinned command's archive implementation."""

import json
import logging
import os
import sys
from pathlib import Path


def require_read_only(event, arguments):
    if event == "open" and arguments[2] & (
        os.O_WRONLY | os.O_RDWR | os.O_CREAT | os.O_TRUNC | os.O_APPEND
    ):
        raise RuntimeError("source oracle attempted to open a file for writing")
    if event in {
        "os.mkdir", "os.remove", "os.rename", "os.rmdir", "os.link", "os.symlink",
        "os.truncate", "os.chmod", "os.chown", "os.utime",
    }:
        raise RuntimeError(f"source oracle attempted filesystem mutation: {event}")


sys.addaudithook(require_read_only)
sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
from hcli.commands.plugin.lint import _lint_plugin_archive
from hcli.lib.console import console

logging.disable(logging.CRITICAL)
console.width = 10000
console.no_color = True
results = []
for filename in json.load(sys.stdin):
    with console.capture() as captured:
        try:
            findings = _lint_plugin_archive(Path(filename).read_bytes(), filename)
            if findings == 0:
                console.print("no recommendations")
            success = True
        except Exception:
            success = False
    results.append({"success": success, "output": captured.get()})
print(json.dumps(results))
