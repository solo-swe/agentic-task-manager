"""`atm agents` (T-442): atm's skills, installed into the agent harnesses."""

import os
import sys
from typing import Optional

import typer

from ...boot.open import open_project
from ...common.core import out, plugin_version, to_json
from ...lib.agents import install_skills, project_folder, skills_status, uninstall_skills
from ..app import MAINTENANCE, app

agents_app = typer.Typer(help="Install atm's skills where your agent harnesses read them (Claude Code for now)")
app.add_typer(agents_app, name="agents", rich_help_panel=MAINTENANCE)

HARNESS_OPTION = typer.Option(None, "--harness", help="Only this harness (claude); all of them without it")
PROJECT_OPTION = typer.Option(False, "--project", help="This project's folder (copied, to commit), not your own")


def _agents_project(project: bool) -> Optional[str]:
    """The project's folder for --project: where the task folder is (the working copy), else this folder."""
    if not project:
        return None
    return project_folder(open_project(os.getcwd())) or os.getcwd()


@agents_app.command("install")
def agents_install(
    ctx: typer.Context,
    harness: Optional[str] = HARNESS_OPTION,
    project: bool = PROJECT_OPTION,
    copy: bool = typer.Option(False, "--copy", help="Copy the skills instead of linking them"),
) -> None:
    """Put atm's skills, this version's, where the harness reads them: linked to atm's folder (copied with --copy or
    --project). Prints every path it wrote; never replaces a folder atm didn't write."""
    result = install_skills(harness, _agents_project(project), copy, plugin_version())
    if ctx.obj.get("json_out", False):
        out(to_json(result))
    else:
        lines = [f"{w['action']}: {w['path']} ({w['how']})" for w in result["written"]]
        changed = [w for w in result["written"] if w["action"] != "unchanged"]
        lines.append(f"{len(changed)} written, {len(result['written']) - len(changed)} unchanged")
        out("\n".join(lines))
    for r in result["refused"]:
        sys.stderr.write(f"atm: left alone: {r['path']} {r['reason']}\n")
    if result["refused"]:
        raise typer.Exit(1)


@agents_app.command("status")
def agents_status(ctx: typer.Context, project: bool = PROJECT_OPTION) -> None:
    """Where atm's skills are for each harness: linked, copied, missing, stale, or someone else's."""
    report = skills_status(_agents_project(project), plugin_version())
    if ctx.obj.get("json_out", False):
        out(to_json(report))
        return
    lines = []
    for h in report:
        here = "installed here" if h["detected"] else "not found here"
        lines.append(f"{h['label']} ({here}), {h['scope']}: {h['folder']}")
        width = max(len(s["skill"]) for s in h["skills"])
        lines += [f"  {s['skill'].ljust(width)}  {s['state']}" for s in h["skills"]]
    out("\n".join(lines))


@agents_app.command("uninstall")
def agents_uninstall(ctx: typer.Context, harness: Optional[str] = HARNESS_OPTION, project: bool = PROJECT_OPTION) -> None:
    """Remove exactly what `atm agents install` wrote, and nothing else."""
    removed = uninstall_skills(harness, _agents_project(project))
    if ctx.obj.get("json_out", False):
        out(to_json({"removed": removed}))
    else:
        out("\n".join(f"removed: {p}" for p in removed) if removed else "nothing atm installed to remove")
