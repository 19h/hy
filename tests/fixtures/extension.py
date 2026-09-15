"""An installed-style extension used with the real pinned HCLI runtime."""

import json
import os

import click

__version__ = "1.2.3"
registration_count = 0
parameter_count = 0


def observe_parameter(_context, _parameter, value):
    global parameter_count
    parameter_count += 1
    return value


def register(cli):
    global registration_count
    registration_count += 1
    if os.environ.get("HY_EXTENSION_TRACE"):
        click.echo(f"registered: {os.getpid()}", err=True)
    if "HY_EXTENSION_EXIT" in os.environ:
        raise SystemExit(int(os.environ["HY_EXTENSION_EXIT"]))
    if os.environ.get("HY_EXTENSION_BROKEN"):
        raise RuntimeError("fixture registration failed")

    @click.group(help="Fixture extension commands.")
    def laboratory():
        pass

    @laboratory.command()
    @click.option("--label", multiple=True, callback=observe_parameter)
    @click.option("--exit-code", type=int, default=0)
    @click.argument("values", nargs=-1, type=click.UNPROCESSED)
    @click.pass_context
    def echo(context, label, exit_code, values):
        """Echo unmodified arguments and the upstream root context."""
        root = context.find_root()
        click.echo(
            json.dumps(
                {
                    "labels": label,
                    "values": values,
                    "auth": root.obj["auth"],
                    "credentials": root.obj["auth_credentials"],
                    "program": root.info_name,
                    "registrations": registration_count,
                    "execution_pid": os.getpid(),
                    "parameter_callbacks": parameter_count,
                }
            )
        )
        context.exit(exit_code)

    @laboratory.command()
    @click.option("--value", prompt="Fixture value")
    def prompt(value):
        click.echo(f"Received: {value}")

    @laboratory.command(hidden=True)
    def hidden():
        click.echo("hidden fixture")

    cli.add_command(laboratory)

    @cli.commands["ida"].command("extension-fixture")
    @click.option("--message", required=True)
    def nested(message):
        """An extension inside a native group."""
        click.echo(message)

    if os.environ.get("HY_EXTENSION_REPLACE"):

        @click.command("whoami", help="Extension identity replacement.")
        def replacement():
            click.echo("extension identity")

        cli.add_command(replacement)

    if os.environ.get("HY_EXTENSION_REMOVE"):
        del cli.commands["license"]
