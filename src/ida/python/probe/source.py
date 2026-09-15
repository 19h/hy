
import sys
import io
import json
import os
import sysconfig

# ensure UTF-8 output for unicode install paths
if hasattr(sys.stdout, "buffer"):
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8")

# PEP 668: distributors mark a Python installation as externally managed by
# dropping this file next to the stdlib. pip refuses to install into such an
# interpreter unless it's a venv, so check for it here rather than shelling
# out to pip just to learn the same thing.
try:
    stdlib = sysconfig.get_path("stdlib")
    externally_managed = bool(stdlib) and os.path.exists(os.path.join(stdlib, "EXTERNALLY-MANAGED"))
except Exception:
    externally_managed = False

print("__hcli__:" + json.dumps({
    "frozen": getattr(sys, "frozen", False),
    "prefix": sys.prefix,
    "base_prefix": sys.base_prefix,
    "executable": sys.executable,
    "virtual_env": os.environ.get("VIRTUAL_ENV"),
    "idapython_venv_executable": os.environ.get("IDAPYTHON_VENV_EXECUTABLE"),
    "version_major": sys.version_info.major,
    "version_minor": sys.version_info.minor,
    "externally_managed": externally_managed,
}))
sys.exit()

