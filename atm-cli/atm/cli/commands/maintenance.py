"""`atm doctor` and `atm service`: the maintenance commands that aren't a group of their own."""

import os
from typing import Optional

import click
import typer

from ...boot.open import open_project
from ...common.core import AtmExit, atm_home, out, to_json
from ...common.dashboard import DEFAULT_PORT
from ...common.errors import AtmFailure
from ...lib.edit import check_project
from ..app import MAINTENANCE, app
from ..serve import open_browser, refresh_server, serve_refresh, serve_start, serve_status, serve_stop


@app.command("doctor", rich_help_panel=MAINTENANCE)
def doctor(
    ctx: typer.Context,
    files: bool = typer.Option(False, "--files", help="Also warn about missing files"),
) -> None:
    """Validate tasks.json: schema, vocabularies, dependencies."""
    result = check_project(open_project(os.getcwd()), files=files)
    errors, warnings = result["errors"], result["warnings"]
    if ctx.obj.get("json_out", False):
        out(to_json({"errors": errors, "warnings": warnings}))
    else:
        for e in errors:
            out(f"error: {e}")
        for w in warnings:
            out(f"warning: {w}")
        if not errors:
            count = result["count"]
            msg = f"tasks.json is fine ({count} tasks"
            if warnings:
                msg += f", {len(warnings)} warnings"
            msg += ")"
            out(msg)
    if errors:
        raise AtmExit(1)


@app.command("service", rich_help_panel=MAINTENANCE)
def service(
    ctx: typer.Context,
    action: str = typer.Argument("start", click_type=click.Choice(["start", "stop", "status"])),
    open_: bool = typer.Option(False, "--open", help="Also open the dashboard in the browser"),
    all_projects: bool = typer.Option(False, "--all", help="Show all projects overview"),
    port: int = typer.Option(DEFAULT_PORT, "--port", help="The port to try first"),
    dir_: Optional[str] = typer.Option(None, "--dir", help="The project to show"),
    refresh: bool = typer.Option(False, "--refresh", help="Only restart if dashboard is older"),
) -> None:
    """Run the dashboard server in the background."""
    cwd = os.path.abspath(dir_) if dir_ else os.getcwd()
    if not os.path.isdir(cwd):
        raise AtmFailure(f"--dir {dir_}: not a directory", 2)
    home = atm_home()
    dry_run = ctx.obj.get("dry_run", False)
    if refresh:
        if dry_run:
            out("[dry-run] would refresh the dashboard server if outdated")
        else:
            serve_refresh(home)
    elif action == "start":
        if dry_run:
            out("[dry-run] would start the dashboard server")
        else:
            refresh_server(home)
            url = serve_start(cwd, port, all_projects)
            if open_:
                open_browser(url)
    elif action == "stop":
        if dry_run:
            out("[dry-run] would stop the dashboard server")
        else:
            serve_stop(home, port)
    else:
        serve_status(home, ctx.obj.get("json_out", False), port)
