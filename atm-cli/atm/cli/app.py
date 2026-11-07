"""The root of atm's command tree: the Typer app every command group in cli/commands/ adds itself to, the options of the
whole tree (`--json`, `--dry-run`), the help's order and panels, and the small helpers the groups share."""

import typer

from ..common.core import out
from ..common.errors import Invalid


# `atm --help` lists the commands people and agents use first (task, loop, claim, vcs, release, component, project,
# sprint), then the maintenance ones, then help and version; what git, hooks and the server call is hidden
# (db merge-driver, project summary). The panels are rich's, which isn't vendored (plugin/scripts/vendor/README.md):
# plain Click help shows the order only.
MAINTENANCE = "Maintenance"
META = "Meta"
HELP_ORDER = ["task", "loop", "claim", "vcs", "release", "component", "project", "sprint", "db", "doctor", "service",
              "agents", "help", "docs", "version"]


class OrderedGroup(typer.core.TyperGroup):
    """The top-level commands in HELP_ORDER (Typer lists commands before groups)."""

    def list_commands(self, ctx):
        names = super().list_commands(ctx)
        return sorted(names, key=lambda n: HELP_ORDER.index(n) if n in HELP_ORDER else len(HELP_ORDER))


app = typer.Typer(
    name="atm",
    help=(
        "atm, the Agentic Task Manager's command line: its commands, by group. "
        "`atm <command> --help` describes one.\n\n"
        "\b\n"
        "Every command takes --json (-j) and --dry-run (-n), anywhere on the line.\n"
        "atm --dir PATH <command> runs as if started in PATH."
    ),
    no_args_is_help=True,
    cls=OrderedGroup,
)


@app.callback()
def _global(
    ctx: typer.Context,
    dry_run: bool = typer.Option(
        False, "--dry-run", "-n", help="Don't execute, just show what would be done"
    ),
    json_out: bool = typer.Option(False, "--json", "-j", help="Print result as JSON"),
) -> None:
    """Global options for every command (they may go anywhere on the line)."""
    ctx.obj = {"dry_run": dry_run, "json_out": json_out}


def _choice(name, value, choices):
    """A value that must be one of `choices` (or not given), refused as atm's argparse refuses it (exit code 2)."""
    if value is not None and value not in choices:
        raise Invalid(f"{name}: invalid choice '{value}' (choose from {', '.join(choices)})")
    return value


def _dry(ctx, what):
    """With --dry-run, say what would be done and stop; otherwise go on."""
    if ctx.obj.get("dry_run", False):
        out(f"[dry-run] would {what}")
        return True
    return False
