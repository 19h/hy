"""Source extraction predicates and ZIP reads, projected into a memory inventory."""

import hashlib
import io
import json
import logging
import sys
import zipfile
from pathlib import Path, PurePosixPath

sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
from hcli.lib.ida.plugin.install import (
    should_extract_plugin_archive_path,
    validate_archive_entry,
)

logging.disable(logging.CRITICAL)

results = []
for case in json.load(sys.stdin):
    try:
        with zipfile.ZipFile(io.BytesIO(Path(case["path"]).read_bytes())) as archive:
            selected = []
            for info in archive.infolist():
                if not should_extract_plugin_archive_path(case["prefix"], info):
                    continue
                relative = PurePosixPath(info.filename).relative_to(
                    case["prefix"].rstrip("/")
                )
                validate_archive_entry(info, relative)
                selected.append((info, relative))
            files = {}
            directories = set()
            for info, relative in selected:
                for parent in relative.parents:
                    if str(parent) != ".":
                        directories.add(str(parent))
                if info.is_dir():
                    directories.add(str(relative))
                else:
                    data = archive.read(info.filename)
                    files[str(relative)] = {
                        "size": len(data),
                        "sha256": hashlib.sha256(data).hexdigest(),
                    }
            results.append(
                {"success": True, "directories": sorted(directories), "files": files}
            )
    except Exception:
        results.append({"success": False})
print(json.dumps(results))
