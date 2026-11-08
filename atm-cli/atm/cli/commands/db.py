"""`atm db`: the task files' maintenance (migrate, merge-driver, archive, unarchive, init)."""

import os
import sys
from typing import Annotated, Optional

import typer

from ...boot.jsongit import merge_versions, migrate_folder, register_project, set_up_store
from ...boot.open import open_project
from ...common.core import AtmExit, out, read_json, to_json
from ...common.errors import Invalid
from ...lib.archive import archive_finished, unarchive_tasks
from ...lib.settings import ensure_project_id, ensure_repository, init_meta
from ..app import MAINTENANCE, app


# How `atm db archive` and `db unarchive` describe the tasks they moved (docs/layered-architecture.md §9, step 12).


def describe_ids(ids, limit=12):
    return ", ".join(ids[:limit]) + (f" and {len(ids) - limit} more" if len(ids) > limit else "")


def describe_kept(kept):
    """ "T-010 (in open PR #1); T-011, T-012 (finished in the last 14 days)": grouped by reason, in first-seen order"""
    groups = {}
    for k in kept:
        groups.setdefault(k["reason"], []).append(k["id"])
    return "; ".join(f"{describe_ids(ids)} ({reason})" for reason, ids in groups.items())


db_app = typer.Typer(help="Database / file maintenance")
app.add_typer(db_app, name="db", rich_help_panel=MAINTENANCE)


@db_app.command("migrate")
def db_migrate(
    ctx: typer.Context,
) -> None:
    """Move tasks.json into .agentic-task-manager/ and set up the folder."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        out("[dry-run] would move tasks.json into .agentic-task-manager/ and set up the folder")
        return
    migration = migrate_folder(os.getcwd())
    if ctx.obj.get("json_out", False):
        out(to_json(migration["result"]))
    else:
        for line in migration["lines"]:
            out(line)


@db_app.command("merge-driver", hidden=True)
def db_merge_driver(
    ctx: typer.Context,
    base: Annotated[str, typer.Argument(help="The common ancestor's tasks.json")],
    ours: Annotated[str, typer.Argument(help="This branch's tasks.json (written with the result)")],
    theirs: Annotated[str, typer.Argument(help="The other branch's tasks.json")],
) -> None:
    """git's merge driver for tasks.json (git runs it; `db init` registers it)."""
    conflicts = merge_versions(base, ours, theirs)
    if ctx.obj.get("json_out", False):
        out(to_json({"conflicts": conflicts}))
    if conflicts:
        sys.stderr.write("atm: tasks.json has conflicting edits (this branch's version was kept):\n")
        for conflict in conflicts:
            sys.stderr.write(f"  {conflict}\n")
        raise AtmExit(1)


@db_app.command("archive")
def db_archive(
    ctx: typer.Context,
    sprint: Optional[list[str]] = typer.Option(None, "--sprint"),
    before: Optional[str] = typer.Option(None, "--before"),
    keep_days: Optional[float] = typer.Option(None, "--keep-days"),
) -> None:
    """Move finished tasks to tasks-archive.json (with --dry-run: say which, and move nothing)."""
    # atm-lib's own dry run says which tasks would move
    dry_run = ctx.obj.get("dry_run", False)
    result = archive_finished(
        open_project(os.getcwd()), sprint, before, keep_days, dry_run
    )
    ids, dropped, kept, sizes = (
        result["moved"],
        result["duplicates_dropped"],
        result["kept"],
        result["sizes"],
    )
    shown = {
        "moved": ids,
        "dry_run": bool(dry_run),
        "tasks_kb": sizes["tasks"] // 1024,
        "archive_kb": sizes["archive"] // 1024,
        "duplicates_dropped": dropped,
        "kept": kept,
    }
    if ctx.obj.get("json_out", False):
        out(to_json(shown))
        return
    if dropped and not dry_run:
        out(
            f"Kept {describe_ids(dropped)} in tasks.json and dropped the archive's copy (an interrupted move)."
        )
    if not ids:
        out("Nothing to archive.")
    elif dry_run:
        out(f"Would archive {len(ids)} task(s): {describe_ids(ids)}.")
    else:
        out(
            f"Archived {len(ids)} task(s): {describe_ids(ids)}. "
            f"tasks.json is now {shown['tasks_kb']} KB, tasks-archive.json {shown['archive_kb']} KB."
        )
    if kept:
        out(
            f"{'Would keep' if dry_run else 'Kept'} {len(kept)} finished: {describe_kept(kept)}."
        )


@db_app.command("unarchive")
def db_unarchive(
    ctx: typer.Context,
    ids: Annotated[list[str], typer.Argument(help="Task IDs")],
) -> None:
    """Bring archived tasks back into tasks.json."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        out(f"[dry-run] would unarchive {', '.join(ids)}")
        return
    result = unarchive_tasks(open_project(os.getcwd()), ids)
    again = result["moves_back"]
    if ctx.obj.get("json_out", False):
        out(to_json(result))
        return
    out(f"Brought back {', '.join(ids)} from the archive.")
    if again:
        out(
            f"{describe_ids(again)} {'is' if len(again) == 1 else 'are'} finished: the next `atm db archive` moves "
            f"{'it' if len(again) == 1 else 'them'} back, unless reopened (`atm task state <id> todo --reason …`)."
        )


@db_app.command("init")
def db_init(
    ctx: typer.Context,
    meta_file: Optional[str] = typer.Option(
        None, "--meta-file", help="Set tasks.json meta from a JSON object"
    ),
) -> None:
    """Give tasks.json a meta.id and register the merge driver."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        out("[dry-run] would give tasks.json a meta.id and register the merge driver")
        return
    project = open_project(os.getcwd())
    if meta_file:
        new = read_json(meta_file)
        if not isinstance(new, dict):
            raise Invalid(f"{meta_file} must hold a JSON object: the meta keys to set")
        created, keys = init_meta(project, new)
        project = open_project(os.getcwd())  # the task data as it is now
        if not ctx.obj.get("json_out", False):
            out(("Created tasks.json; meta: " if created else "Updated tasks.json meta: ") + (", ".join(keys) or "-"))
    project_id = ensure_project_id(project)
    repository = ensure_repository(project)
    register_project(project)  # the dashboard lists every project atm runs in
    setup = set_up_store(project)
    if ctx.obj.get("json_out", False):
        out(
            to_json(
                {
                    "project_id": project_id,
                    "repository": repository,
                    "git": setup["git"],
                    "gitattributes_added": setup["gitattributes_added"],
                }
            )
        )
        return
    if project_id:
        out(f"Gave this project an id ({project_id}) in tasks.json meta, for the dashboard's saved view.")
    if repository:
        out(f"Recorded the repository address ({repository}), for commit and PR links on the dashboard.")
    if not setup["git"]:
        out("Not in a git repository: there is no merge driver to set up.")
        return
    out(
        "Registered the tasks.json merge driver (git config merge.atm)"
        + (
            " and added .agentic-task-manager/.gitattributes (staged)."
            if setup["gitattributes_added"]
            else "; .gitattributes already set."
        )
    )
