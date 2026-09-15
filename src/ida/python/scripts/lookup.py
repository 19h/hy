
import io
import json
import os
import sys
import sysconfig

# ensure UTF-8 output for unicode install paths
if hasattr(sys.stdout, "buffer"):
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8")

name = sys.argv[1]

if os.name == "nt":
    filenames = [name + ".exe", name + ".cmd", name + ".bat", name, name + "-script.py"]
else:
    filenames = [name]

# Scan distributions for the entry point, rather than using entry_points(name=...)
# or EntryPoint.dist, which need Python 3.10+. IDA may be using an older interpreter.
entry_point = None
record_candidates = []
try:
    from importlib.metadata import distributions

    for dist in distributions():
        try:
            eps = [
                ep
                for ep in dist.entry_points
                if ep.name == name and ep.group in ("console_scripts", "gui_scripts")
            ]
        except Exception:
            continue

        if not eps:
            continue

        entry_point = {
            "name": eps[0].name,
            "value": eps[0].value,
            "group": eps[0].group,
            "distribution": dist.metadata["Name"],
            "version": dist.version,
        }

        # `files` is None when the distribution has no RECORD
        for file in dist.files or []:
            if os.path.basename(str(file)) not in filenames:
                continue
            try:
                record_candidates.append(os.path.realpath(str(dist.locate_file(file))))
            except Exception:
                pass

        break
except Exception:
    pass

scripts_dirs = []


def add_dir(path):
    if path and path not in scripts_dirs:
        scripts_dirs.append(path)


# next to the interpreter: where a virtualenv keeps its scripts.
# don't resolve symlinks here: a virtualenv's python is usually a link to the base
# interpreter, and following it would lead out of the environment we're inspecting.
add_dir(os.path.dirname(os.path.abspath(sys.executable)))
add_dir(sysconfig.get_path("scripts"))
try:
    # where `pip install --user` puts scripts
    add_dir(sysconfig.get_path("scripts", sysconfig.get_preferred_scheme("user")))
except Exception:
    add_dir(sysconfig.get_path("scripts", "nt_user" if os.name == "nt" else "posix_user"))

candidates = record_candidates + [os.path.join(d, f) for d in scripts_dirs for f in filenames]

path = None
for candidate in candidates:
    if os.path.isfile(candidate):
        path = candidate
        break

print("__hcli__:" + json.dumps({
    "name": name,
    "path": path,
    "scripts_dirs": scripts_dirs,
    "record_candidates": record_candidates,
    "entry_point": entry_point,
}))

