"""`atm release`: releases (list, add, edit, archive, notes)."""

import os
from typing import Annotated, Optional

import typer

from ...boot.open import open_project
from ...common.core import out, to_json
from ...lib.releases import add_release, archive_release, edit_release, list_releases, release_notes
from ..app import app


# How `atm release list` and `release notes` print releases (docs/layered-architecture.md §9, step 11).


def describe_release(r):
    details = [r["state"]] + ([f"target {r['target']}"] if r.get("target") else [])
    if r.get("released_at"):
        details.append(f"released {r['released_at'][:10]}")
    if isinstance(r.get("pr"), dict) and r["pr"].get("number"):
        details.append(f"PR #{r['pr']['number']}")
    title = r["id"] + (f" “{r['name']}”" if r.get("name") else "")
    kinds = ", ".join(f"{k} {n}" for k, n in r["by_kind"].items())
    stages = ", ".join(f"{s.replace('_', ' ')} {n}" for s, n in r["by_stage"].items())
    line = f"{title} · {' · '.join(details)}: {r['tasks']} task{'' if r['tasks'] == 1 else 's'}"
    return line + (f" ({kinds})" if kinds else "") + (f"; {stages}" if stages else "")


def describe_notes(notes):
    """The draft as Markdown: a heading per type, a line per task; what's not done yet at the end."""
    release = notes["release"]
    lines = [f"# {release['id']}" + (f" “{release['name']}”" if release.get("name") else "")]
    if not notes["groups"]:
        lines += ["", "Nothing done in it yet."]
    for group in notes["groups"]:
        lines += ["", f"## {group['label']}", ""]
        for t in group["tasks"]:
            lines.append(f"- **{t['id']}** {t['name']}" + (f": {t['outcome']}" if t["outcome"] else ""))
    if notes["open"]:
        lines += ["", "## Not done yet", ""]
        lines += [f"- {t['id']} {t['name']} ({t['status']})" for t in notes["open"]]
    return lines


release_app = typer.Typer(help="Release management")
app.add_typer(release_app, name="release")


@release_app.command("list")
def release_list(
    ctx: typer.Context,
    all_: bool = typer.Option(False, "--all", help="List archived releases too"),
) -> None:
    """List releases with their tasks by kind and stage."""
    project = open_project(os.getcwd())
    listed = list_releases(project, all_)
    if ctx.obj.get("json_out", False):
        out(to_json(listed["releases"]))
        return
    if not listed["releases"]:
        out(
            "No open releases (--all lists the archived ones)."
            if listed["has_releases"]
            else "No releases yet: atm release add <R>."
        )
        return
    for r in listed["releases"]:
        out(describe_release(r))


@release_app.command("add")
def release_add(
    ctx: typer.Context,
    release_id: Annotated[str, typer.Argument(help="The release's ID (letters, digits, '.', '-', '_')")],
    name: Optional[str] = typer.Option(None, "--name", help="Add, edit: a name besides its ID (empty: remove it)"),
    target: Optional[str] = typer.Option(None, "--target", metavar="DATE", help="Add, edit: the planned date, YYYY-MM-DD"),
    pr: Optional[list[int]] = typer.Option(None, "--pr", metavar="N", help="Add: give it the tasks of pull request N (repeatable)"),
    released_at: Optional[str] = typer.Option(None, "--released-at", metavar="TIME", help="Add: it shipped already, at this time (e.g. its PR's merge)"),
) -> None:
    """Add a release."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        parts = [release_id]
        if name:
            parts.append(f"--name {name}")
        if target:
            parts.append(f"--target {target}")
        if pr:
            for n in pr:
                parts.append(f"--pr {n}")
        if released_at:
            parts.append(f"--released-at {released_at}")
        out(f"[dry-run] would add release {' '.join(parts)}")
        return
    project = open_project(os.getcwd())
    result = add_release(
        project, release_id, name=name, target=target, prs=pr or [], released_at=released_at
    )
    if ctx.obj.get("json_out", False):
        out(to_json(result))
        return
    release, given, prs = result["release"], result["tasks"], pr or []
    line = f'Added the release “{release_id}” ({release["state"]})'
    if given:
        numbers = ", ".join(f"#{n}" for n in prs)
        line += f", with the {len(given)} task{'' if len(given) == 1 else 's'} of PR{'s' if len(prs) > 1 else ''} {numbers}"
    out(line + ".")


@release_app.command("edit")
def release_edit(
    ctx: typer.Context,
    release_id: Annotated[str, typer.Argument(help="The release's ID (letters, digits, '.', '-', '_')")],
    name: Optional[str] = typer.Option(None, "--name", help="Add, edit: a name besides its ID (empty: remove it)"),
    target: Optional[str] = typer.Option(None, "--target", metavar="DATE", help="Add, edit: the planned date, YYYY-MM-DD"),
) -> None:
    """Edit a release."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        parts = [release_id]
        if name is not None:
            parts.append(f"--name {name}")
        if target is not None:
            parts.append(f"--target {target}")
        out(f"[dry-run] would edit release {' '.join(parts)}")
        return
    project = open_project(os.getcwd())
    result = edit_release(project, release_id, name=name, target=target)
    release, changed = result["release"], result["changed"]
    if ctx.obj.get("json_out", False):
        out(to_json(release))
    else:
        out(f"{release['id']}: changed {', '.join(changed)}" if changed else f"{release['id']}: nothing changed")


@release_app.command("archive")
def release_archive(
    ctx: typer.Context,
    release_id: Annotated[str, typer.Argument(help="The release's ID (letters, digits, '.', '-', '_')")],
) -> None:
    """Archive a release."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        out(f"[dry-run] would archive release {release_id}")
        return
    project = open_project(os.getcwd())
    release = archive_release(project, release_id)
    if ctx.obj.get("json_out", False):
        out(to_json(release))
    else:
        out(f"Archived the release “{release['id']}”.")


@release_app.command("notes")
def release_notes_cmd(
    ctx: typer.Context,
    release_id: Annotated[str, typer.Argument(help="The release's ID (letters, digits, '.', '-', '_')")],
) -> None:
    """Print a Markdown draft of a release's changes."""
    project = open_project(os.getcwd())
    notes = release_notes(project, release_id)
    if ctx.obj.get("json_out", False):
        out(to_json(notes))
        return
    for line in describe_notes(notes):
        out(line)
