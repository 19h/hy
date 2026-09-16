"""Read-only source search model dumping, print_json and Unix stdout encoding."""

import io
import json
import os
import sys
from pathlib import Path
from unittest.mock import patch


def deny_mutation(event, arguments):
    write_flags = os.O_WRONLY | os.O_RDWR | os.O_CREAT | os.O_TRUNC | os.O_APPEND
    if event == "open" and arguments[2] & write_flags:
        raise RuntimeError("oracle attempted a file write")
    if event in {
        "os.mkdir", "os.remove", "os.rename", "os.rmdir", "os.link",
        "os.symlink", "os.truncate", "os.chmod", "os.chown", "os.utime",
    }:
        raise RuntimeError(f"oracle attempted mutation: {event}")


sys.addaudithook(deny_mutation)
sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
from rich.console import Console
import hcli.lib.console as console_module
from hcli.commands.plugin.search import PluginExactVersionQueryResult, _dump_result

case = json.load(sys.stdin)
results = []
for document in case["urls"]:
    url = json.loads(document)
    report = PluginExactVersionQueryResult(plugin=case["plugin"], download_locations=[{
        "ida_versions": "9.0", "platforms": "linux-x86_64", "url": url,
    }])
    output = io.StringIO()
    with patch.object(console_module, "console", Console(file=output, force_terminal=False)):
        console_module.print_json(_dump_result(report))
    try:
        stdout = list(url.encode("utf-8", "surrogateescape"))
    except UnicodeEncodeError:
        stdout = None
    results.append({"json": output.getvalue(), "stdout": stdout})
print(json.dumps(results))
