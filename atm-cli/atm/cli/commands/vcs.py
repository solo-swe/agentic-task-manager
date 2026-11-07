"""`atm vcs`: version control (commit, pr, pushed, repository)."""

import os
from typing import Annotated, Optional

import typer

from ...boot.identity import agent_identity
from ...boot.open import open_project
from ...common.core import out, to_json
from ...common.errors import Invalid
from ...lib.code import (
    commit_task,
    folder_changes,
    record_commit,
    record_merge,
    record_pr,
    repository_paths,
)
from ...lib.settings import get_repository, set_repository
from ..app import app

vcs_app = typer.Typer(help="Version control integration")
app.add_typer(vcs_app, name="vcs")


@vcs_app.command("commit")
def vcs_commit(
    ctx: typer.Context,
    task_id: Annotated[str, typer.Argument(help="The task ID")],
    file: Optional[list[str]] = typer.Option(
        None, "--file", help="A file the task changed (repeatable)"
    ),
    dir_: Optional[list[str]] = typer.Option(
        None, "--dir", help="Every change under a folder (repeatable)"
    ),
    message: Optional[str] = typer.Option(None, "--message", help="The commit message"),
    record: Optional[str] = typer.Option(
        None, "--record", help="Record this existing commit instead"
    ),
    agent: Optional[str] = typer.Option(None, "--agent"),
) -> None:
    """Commit exactly the named files plus tasks.json, and record the commit."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        parts = [f"atm vcs commit {task_id}"]
        if file:
            for f in file:
                parts.append(f"--file {f}")
        if dir_:
            for d in dir_:
                parts.append(f"--dir {d}")
        if message:
            parts.append(f"--message {message}")
        if record:
            parts.append(f"--record {record}")
        if agent:
            parts.append(f"--agent {agent}")
        out("[dry-run] would run: " + " ".join(parts))
        return
    project = open_project(os.getcwd(), agent_identity(agent))
    if record is not None:
        if file or dir_ or message is not None:
            raise Invalid("--record records an existing commit: no --file, --dir or --message")
        result = record_commit(project, task_id, record)
        if ctx.obj.get("json_out", False):
            shown = {k: result[k] for k in ("task", "commit", "recorded")}
            out(to_json({**shown, **({"hint": result["hint"]} if result["hint"] else {})}))
            return
        out(f"{result['task']}: recorded {result['commit'][:7]} {result['summary']}".rstrip())
        if result["hint"]:
            out(f"  {result['hint']}")
        return
    files = repository_paths(project, file or [], os.getcwd())
    if dir_:
        files += [f for f in folder_changes(project, task_id, dir_, os.getcwd()) if f not in files]
    result = commit_task(project, task_id, files, message)
    if ctx.obj.get("json_out", False):
        out(to_json(result))
        return
    out(f"{result['task']}: committed {result['commit'][:7]} {result['message'].splitlines()[0]}")
    out("  " + ", ".join(result["files"]))


@vcs_app.command("pr")
def vcs_pr(
    ctx: typer.Context,
    ids: Annotated[list[str], typer.Argument(help="Task IDs")],
    number: int = typer.Option(..., "--number", help="The PR number"),
    url: str = typer.Option(..., "--url", help="The PR's URL"),
    agent: Optional[str] = typer.Option(None, "--agent"),
) -> None:
    """Record the PR that contains the tasks."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        parts = [f"atm vcs pr {' '.join(ids)} --number {number} --url {url}"]
        if agent:
            parts.append(f"--agent {agent}")
        out("[dry-run] would run: " + " ".join(parts))
        return
    project = open_project(os.getcwd(), agent_identity(agent))
    task_ids = record_pr(project, ids, number, url)
    if ctx.obj.get("json_out", False):
        out(to_json({"tasks": task_ids, "pr": {"number": number, "url": url}}))
    else:
        out(f"{', '.join(task_ids)}: in PR #{number} ({url})")


@vcs_app.command("pushed")
def vcs_pushed(
    ctx: typer.Context,
    pr: int = typer.Option(..., "--pr", help="The PR number"),
    at: Optional[str] = typer.Option(None, "--at", help="When it was merged (ISO 8601 UTC)"),
    agent: Optional[str] = typer.Option(None, "--agent"),
) -> None:
    """Record that a PR was merged, on the tasks it contains."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        parts = [f"atm vcs pushed --pr {pr}"]
        if at:
            parts.append(f"--at {at}")
        if agent:
            parts.append(f"--agent {agent}")
        out("[dry-run] would run: " + " ".join(parts))
        return
    project = open_project(os.getcwd(), agent_identity(agent))
    result = record_merge(project, pr, at)
    if ctx.obj.get("json_out", False):
        out(to_json(result))
        return
    task_ids = result["tasks"]
    out(f"PR #{pr} merged: {', '.join(task_ids)}" if task_ids else f"PR #{pr}: already recorded as merged")
    for release_id in result["released"]:
        out(f"Released {release_id}: all its tasks are merged in PR #{pr}.")
    for release_id, waiting in result["pending"].items():
        out(f"{release_id} stays planned: {', '.join(waiting)} {'is' if len(waiting) == 1 else 'are'} not merged in PR #{pr}.")


@vcs_app.command("repository")
def vcs_repository(
    ctx: typer.Context,
    url: Optional[str] = typer.Argument(None, help="An http(s) URL or git remote"),
    old: Optional[str] = typer.Option(
        None, "--from", help="Move recorded PR links from this address"
    ),
    agent: Optional[str] = typer.Option(None, "--agent"),
) -> None:
    """Print or set the repository's web address."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        parts = ["atm vcs repository"]
        if url:
            parts.append(url)
        if old:
            parts.append(f"--from {old}")
        if agent:
            parts.append(f"--agent {agent}")
        out("[dry-run] would run: " + " ".join(parts))
        return
    project = open_project(os.getcwd(), agent_identity(agent))
    if url is None:
        if old is not None:
            raise Invalid(f"unexpected argument '{old}' (atm vcs repository URL)")
        current = get_repository(project)
        if ctx.obj.get("json_out", False):
            out(to_json({"repository": current}))
        elif current:
            out(current)
        else:
            out("No repository address yet. Set it with `atm vcs repository <url>` (a git remote works too).")
        return
    result = set_repository(project, url, old)
    moved = result["moved_links"]
    if ctx.obj.get("json_out", False):
        out(to_json({"repository": result["repository"], "moved_links": moved}))
    else:
        links = f" ({moved} PR link{'' if moved == 1 else 's'} moved from {result['previous']})" if moved else ""
        out(f"Repository address set: {result['repository']}{links}")
