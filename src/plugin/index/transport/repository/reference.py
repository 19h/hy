"""Read-only upstream repository policy with deterministic in-memory responses."""

import json
import os
import sys
from pathlib import Path
from unittest.mock import patch


class ForbiddenMutation(BaseException):
    pass


def require_read_only(event, arguments):
    if event == "open" and arguments[2] & (
        os.O_WRONLY | os.O_RDWR | os.O_CREAT | os.O_TRUNC | os.O_APPEND
    ):
        raise ForbiddenMutation("source oracle attempted a file write")
    if event in {
        "os.mkdir", "os.remove", "os.rename", "os.rmdir", "os.link",
        "os.symlink", "os.truncate", "os.chmod", "os.chown", "os.utime",
    }:
        raise ForbiddenMutation(f"source oracle attempted filesystem mutation: {event}")


sys.addaudithook(require_read_only)
sys.path.insert(0, str(Path(sys.argv[1]) / "src"))
import httpx
import platformdirs

# Rust owns this fixture. Importing upstream must not migrate the user's config.
with patch.object(platformdirs, "user_config_dir", return_value=sys.argv[2]):
    import hcli.lib.auth as auth
import hcli.lib.ida.plugin.repo as repository
from hcli.lib.ida.plugin.exceptions import PluginAccessDeniedError

request = json.load(sys.stdin)
result = []
if "hosts" in request:
    for value in request["hosts"]:
        try:
            result.append(repository.is_plugin_repo_host(value))
        except Exception:
            result.append(None)
else:
    client_type = httpx.Client
    for case in request["cases"]:
        events = []
        calls = [0]

        def resolve():
            calls[0] += 1
            return {"x-api-key": "fixture-key"} if case["authenticated"] else {}

        def respond(request):
            reply = case["replies"][len(events)]
            names = [
                "accept", "accept-encoding", "connection",
                "x-api-key", "authorization", "cookie",
            ]
            events.append({
                "url": str(request.url),
                "headers": {name: request.headers.get(name) for name in names},
                "user_agent": "user-agent" in request.headers,
            })
            return httpx.Response(
                reply["status"],
                headers=[(name.encode(), bytes(value)) for name, value in reply["headers"]],
                content=bytes(reply["body"]),
            )

        client = client_type(transport=httpx.MockTransport(respond), follow_redirects=False)
        with (
            patch.object(repository.httpx, "Client", return_value=client),
            patch.object(auth, "get_optional_auth_headers", side_effect=resolve),
        ):
            try:
                body = repository.fetch_plugin_repo_bytes(case["url"], repo_name=case["repository"])
                outcome = {"kind": "success", "body": list(body)}
            except PluginAccessDeniedError as error:
                outcome = {
                    "kind": "denied", "status": error.status_code, "url": error.url,
                    "authenticated": error.authenticated, "repository": error.repo_name,
                    "message": str(error),
                }
            except httpx.HTTPStatusError as error:
                outcome = {
                    "kind": "http", "status": error.response.status_code,
                    "url": str(error.request.url),
                }
            except httpx.DecodingError:
                outcome = {"kind": "decode"}
            except ValueError as error:
                if str(error).startswith("too many redirects"):
                    outcome = {"kind": "limit"}
                elif str(error).startswith("HTTPS request was redirected"):
                    outcome = {"kind": "downgrade"}
                else:
                    raise
        result.append({"requests": events, "auth_calls": calls[0], "result": outcome})
print(json.dumps(result))
