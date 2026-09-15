"""Load existing hcli.extensions against their actual Python Click host.

The control connection is separate from stdin/stdout, so prompts and extension
output retain their normal terminal streams. Registration runs once per process.
"""

import importlib
import importlib.metadata
import json
import os
import socket
import struct
import sys


def command_state(command):
    """Detect replaced callbacks, parameters and command attributes without parsing."""
    display_keys = {"help", "short_help", "hidden", "epilog"}
    attributes = {
        key: value
        for key, value in vars(command).items()
        if key != "commands" and key not in display_keys
    }
    parameters = [vars(parameter).copy() for parameter in command.params]
    display = {key: getattr(command, key, None) for key in sorted(display_keys)}
    return {
        "execution": (id(command), repr(attributes), repr(parameters)),
        "display": repr(display),
    }


def snapshot(command, path=()):
    states = {path: command_state(command)}
    for name, child in getattr(command, "commands", {}).items():
        states.update(snapshot(child, (*path, name)))
    return states


def describe(name, command):
    return {
        "name": name,
        "help": command.short_help or command.help or "",
        "hidden": command.hidden,
        "owned": True,
        "children": [
            describe(name, child)
            for name, child in getattr(command, "commands", {}).items()
        ],
    }


def changed_commands(command, before, path=()):
    changed = []
    for name, child in getattr(command, "commands", {}).items():
        child_path = (*path, name)
        if before.get(child_path) != command_state(child):
            changed.append(describe(name, child))
            continue
        descendants = changed_commands(child, before, child_path)
        if descendants:
            entry = describe(name, child)
            entry["owned"] = False
            entry["children"] = descendants
            changed.append(entry)
    return changed


def command_argument(command, arguments):
    """Locate a base group's child without invoking Click parameter callbacks."""
    options = {
        spelling: parameter
        for parameter in command.params
        for spelling in (
            *getattr(parameter, "opts", ()),
            *getattr(parameter, "secondary_opts", ()),
        )
    }
    index = 0
    while index < len(arguments):
        argument = arguments[index]
        if argument == "--":
            return index + 1
        if not argument.startswith("-") or argument == "-":
            return index
        spelling, separator, _ = argument.partition("=")
        parameter = options.get(spelling)
        attached = bool(separator)
        if parameter is None and len(argument) > 2 and not argument.startswith("--"):
            parameter = options.get(argument[:2])
            attached = True
        if parameter is None:
            return None
        values = 0 if parameter.is_flag else parameter.nargs
        index += 1 + max(0, values - int(attached))
    return None


def selects_extension(command, arguments, before, path=()):
    previous = before.get(path)
    if previous is None or previous["execution"] != command_state(command)["execution"]:
        return True
    children = getattr(command, "commands", {})
    if not hasattr(command, "commands"):
        return False
    index = command_argument(command, arguments)
    if index is None or index >= len(arguments):
        return False
    name = arguments[index]
    child = children.get(name)
    if child is None:
        return (*path, name) in before
    return child is not None and selects_extension(
        child, arguments[index + 1 :], before, (*path, name)
    )


def load_extensions():
    entries = list(importlib.metadata.entry_points().select(group="hcli.extensions"))
    if not entries:
        return None, {}, {"extensions": [], "commands": [], "removed": []}

    # Import the normal host with registration deferred, retaining its actual
    # groups, callbacks, configuration and authentication context for extensions.
    extensions = importlib.import_module("hcli.lib.extensions")
    get_extensions = extensions.get_extensions
    extensions.get_extensions = list
    try:
        host = importlib.import_module("hcli.main")
    finally:
        extensions.get_extensions = get_extensions
    host.get_extensions = get_extensions
    before = snapshot(host.cli)
    original_help = host.cli.help
    installed = get_extensions()
    for extension in installed:
        extension["function"](host.cli)
    if host.cli.help == original_help:
        host.cli.help = host.get_help_text()
    after = snapshot(host.cli)
    catalog = {
        "extensions": [
            {"name": extension["name"], "version": str(extension["version"])}
            for extension in installed
        ],
        "commands": changed_commands(host.cli, before),
        "removed": [path for path in before if path and path not in after],
    }
    return host.cli, before, catalog


def main():
    port, token, *arguments = sys.argv[1:]
    with socket.create_connection(("127.0.0.1", int(port))) as connection:
        try:
            command, before, catalog = load_extensions()
            selected = command is not None and selects_extension(
                command, arguments, before
            )
            result = {"status": "ready", "selected": selected, "catalog": catalog}
        except Exception as error:  # noqa: BLE001 - Serialize arbitrary extension failures for the host.
            result = {"status": "failed", "message": f"{type(error).__name__}: {error}"}
        report = json.dumps({"token": token, **result}).encode("utf-8")
        connection.sendall(struct.pack("!I", len(report)) + report)
        with connection.makefile("rb") as control:
            decision = control.readline()
    if result["status"] == "failed":
        sys.exit(1)
    if decision == b"run\n" and selected:
        command.main(args=arguments, prog_name=os.environ.get("HCLI_BINARY_NAME", "hy"))


if __name__ == "__main__":
    main()
