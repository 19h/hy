# Python extension compatibility

The selected compatibility model is existing Python extensions hosted in a
configured interpreter, using `HCLI_EXTENSION_PYTHON`.

Hy discovers the `hcli.extensions` entry-point group in a selected Python
interpreter. That interpreter must have the extension packages and a compatible
`ida-hcli` package installed. The current bridge is tested against upstream
revision `92df5b10c8b2e860a10bbfd8595c8bc2b7c3b64f` (package version 0.24.0).
It does not install or update a runtime automatically.

Set the interpreter explicitly when extensions are installed in a virtual
environment:

```sh
export HCLI_EXTENSION_PYTHON=/absolute/path/to/venv/bin/python
hy extension list
```

When this variable is absent, Hy searches `PATH` for `python3`, then `python`.
When it is present but empty, Python extension discovery is disabled. Without
an available interpreter, the native command tree remains usable. An explicitly
configured interpreter that cannot start produces an error with its path.

`hy extension create` prints the upstream template command. It does not create
files and does not accept a project-name argument.

## Command ownership and execution

The bridge imports the actual Python HCLI host, defers its automatic extension
registration, and takes a snapshot of its command tree. It then loads and
registers the installed extensions once. New or replaced commands, changed
execution attributes, and removed command paths determine which invocations
the interpreter handles. Changes to an existing group's execution attributes
delegate that group and its descendants to the Python host. This preserves
the group's callback and context, including root authentication options.

The same process that registers an extension executes its selected command.
Parameters are not parsed during ownership inspection, so Click callbacks run
only during the actual invocation. Standard input, output and error are inherited;
interactive prompts keep their controlling terminal. Normal exit codes, including
an extension's `SystemExit` during registration, propagate to the caller.

Native commands receive extension metadata for help and command inventory, then
execute in Rust after the interpreter exits. Merely changing help/display
attributes does not transfer command execution to Python. Installed extensions
are loaded during each invocation, including help, matching upstream startup
behavior. Consequently, entry-point imports and registration can have the same
side effects as running them in HCLI. An empty entry-point inventory does not
import HCLI or its configuration modules.

The control channel is a local TCP connection with a per-process session token.
Its JSON report has a 1 MiB limit and is separate from terminal streams. There is
no registration deadline; arbitrary extension code can block startup as it can
in upstream HCLI. Token staging is temporary and removed after the invocation.

## Verification and limits

The runtime-dependent tests use temporary distribution metadata and the actual
installed Python HCLI/Click packages. They cover entry-point names and versions,
native help/inventory, nested commands, overrides/removals, literal arguments,
authentication context, exit status, terminal prompts, failed registration,
interpreter discovery and unchanged native command dispatch.

To reproduce against the requested checkout:

```sh
uv venv /tmp/hy-extension-runtime --python 3.13
uv pip install --python /tmp/hy-extension-runtime/bin/python /Users/int/dev/ida-hcli
HY_TEST_EXTENSION_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --locked --test extensions -- --include-ignored
```

These tests are opt-in because the native test suite does not require an
installed Python HCLI environment. Their verified runtime used Python 3.13.15,
ida-hcli 0.24.0 and Click 8.5.0. Two runtime-configuration tests also run in the
default suite without Python dependencies.

Windows runtime execution, arbitrary lazy command groups, custom parser behavior,
in-place mutation of callback internals or shared globals, and custom root-help
rewrites require further coverage. The ownership snapshot observes command and
parameter attributes; it cannot establish semantic equivalence for arbitrary
Python object-graph mutation. Native help also retains Clap's presentation rather
than rendering the full Rich help document. This bridge does not implement a
separate native shared-library extension ABI.
