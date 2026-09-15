"""Read-only source oracle. All apparent file mutations use MemoryFile bytes."""

import dataclasses
import io
import json
import os
import sys
import types
from pathlib import Path


class MemoryFile:
    def __init__(self, data):
        self.data = None if data is None else bytes(data)
        self.parent = self

    def __str__(self):
        return "<path>"

    def is_file(self):
        return self.data is not None

    def mkdir(self, **kwargs):
        pass

    def read_text(self, **kwargs):
        text = self.data.decode("utf-8", errors="replace")
        return text.replace("\r\n", "\n").replace("\r", "\n")

    def write_text(self, text, **kwargs):
        self.data = text.replace("\n", os.linesep).encode("utf-8")

    def open(self, *args, **kwargs):
        owner = self

        class Append(io.StringIO):
            def __exit__(self, *args):
                text = self.getvalue().replace("\n", os.linesep)
                owner.data = (owner.data or b"") + text.encode("utf-8")

        return Append()


def plan_result(module, case):
    module.is_windows = lambda: case["system"] == "Windows"
    module.is_macos = lambda: case["system"] == "Macos"
    module.has_systemd_user = lambda: case["systemd"]
    environment = {} if case["shell"] is None else {"SHELL": case["shell"]}
    session_variables = {
        "wayland": {"WAYLAND_DISPLAY": "fixture"},
        "x11": {"DISPLAY": "fixture"},
        "tty": {"TERM": "fixture"},
        "unknown": {},
    }
    environment.update(session_variables[case["session"]])
    module.os = types.SimpleNamespace(environ=environment)
    plan = module.build_configuration_plan(
        "IDAPYTHON_VENV_EXECUTABLE", case["value"], home=Path("/fixture/home")
    )
    return {
        "plan": dataclasses.asdict(plan),
        "needs_logout": plan.needs_logout,
        "shell": module.detect_login_shell(),
    }


def session_result(module, case):
    variables = {
        "WAYLAND_DISPLAY": case["wayland"],
        "DISPLAY": case["display"],
        "TERM": case["term"],
    }
    module.os = types.SimpleNamespace(
        environ={name: value for name, value in variables.items() if value is not None}
    )
    return module.detect_session_type()


def file_result(module, case):
    file = MemoryFile(case["initial"])
    step = module.ConfigurationStep(case["kind"], "", file, case["content"], None, False)
    result = module.execute_step(step)
    return {
        "skipped": result.skipped,
        "data": list(file.data),
        "message": result.message,
    }


path = Path(sys.argv[1]) / "src/hcli/lib/ida/python/platform_env.py"
module = types.ModuleType("source_platform")
sys.modules[module.__name__] = module
exec(compile(path.read_text(), str(path), "exec"), module.__dict__)
evaluate = {"plans": plan_result, "sessions": session_result, "files": file_result}[sys.argv[2]]
results = [evaluate(module, case) for case in json.load(sys.stdin)]
print(json.dumps(results, default=str))
