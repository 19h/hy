"""Read Rust-owned fixtures through the actual source recognition and info functions."""

import importlib
import io
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
from rich.console import Console
from hcli.lib.ida.plugin.repo.bundle import is_plugin_bundle_zip

bundle = importlib.import_module("hcli.commands.plugin.bundle")
path = Path(sys.argv[2])
recognized = is_plugin_bundle_zip(path)
stream = io.StringIO()
bundle.console = Console(file=stream, width=10_000, color_system=None, highlight=False)
success = False
if recognized:
    try:
        bundle.info.callback(str(path))
        success = True
    except Exception:
        pass
print(json.dumps({"recognized": recognized, "success": success, "report": stream.getvalue()}))
