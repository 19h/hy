"""Run the pinned pip helpers with an in-memory subprocess recorder."""

import ast
import dataclasses
import json
import sys
from pathlib import Path
from types import SimpleNamespace

path = Path(sys.argv[1]) / "src/hcli/lib/ida/python/__init__.py"
names = {
    "CantInstallPackagesError", "PipOptions", "merge_bundle_pip_options", "_format_pip_error",
    "externally_managed_environment_message", "_raise_for_known_pip_errors",
    "verify_pip_can_install_packages", "pip_install_packages",
}
tree = ast.parse(path.read_text())
definitions = [node for node in tree.body if getattr(node, "name", None) in names]
context = {
    "__name__": "__main__", "dataclass": dataclasses.dataclass, "Path": Path,
    "logger": SimpleNamespace(debug=lambda *args: None),
}
future = ast.ImportFrom(module="__future__", names=[ast.alias(name="annotations")], level=0)
for node in definitions:
    if isinstance(node, ast.FunctionDef) and node.name in {"verify_pip_can_install_packages", "pip_install_packages"}:
        context["PIP_OPTIONS_DEFAULT"] = context["PipOptions"]()
    module = ast.fix_missing_locations(ast.Module(body=[future, node], type_ignores=[]))
    exec(compile(module, str(path), "exec"), context)

results = []
for case in json.load(sys.stdin):
    context["ENV"] = SimpleNamespace(HCLI_BINARY_NAME=case.get("binary", "hy"))
    calls = []

    def run(argv, **kwargs):
        assert kwargs == {"capture_output": True, "check": False}
        calls.append(argv)
        return SimpleNamespace(
            returncode=1 if case["mode"] == "error" else 0,
            stdout=bytes(case.get("stdout", [])), stderr=bytes(case.get("stderr", [])),
        )

    context["subprocess"] = SimpleNamespace(run=run)
    if case["mode"] == "plan":
        values = dict(case["options"])
        values["offline"] = values.pop("no_index")
        values.pop("skip_environment_check")
        values["extra_index_urls"] = tuple(values["extra_index_urls"])
        values["find_links"] = tuple(values["find_links"])
        options = context["PipOptions"](**values)
        custom = options.has_custom_sources
        if case["bundled"]:
            bundle = context["PipOptions"](
                isolated=True, no_cache_dir=True, disable_pip_version_check=True,
                find_links=("/bundle wheels",),
            )
            options = context["merge_bundle_pip_options"](options, bundle)
        operation = "verify_pip_can_install_packages" if case["resolve"] else "pip_install_packages"
        context[operation](Path("/fixture/python"), case["dependencies"], options)
        results.append({"argv": calls[0], "custom_sources": custom})
    else:
        try:
            context["pip_install_packages"](Path("/fixture/python"), ["fixture"])
        except context["CantInstallPackagesError"] as error:
            results.append(str(error))
        else:
            raise AssertionError("Expected a pip failure")
print(json.dumps(results))
