
import sys
from importlib.metadata import distributions

name = sys.argv[1]

for dist in distributions():
    try:
        eps = [
            ep
            for ep in dist.entry_points
            if ep.name == name and ep.group in ("console_scripts", "gui_scripts")
        ]
    except Exception:
        continue

    if eps:
        sys.argv = [name] + sys.argv[2:]
        sys.exit(eps[0].load()())

sys.exit("error: no console script named " + name)

