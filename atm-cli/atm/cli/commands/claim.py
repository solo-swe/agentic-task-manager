"""`atm claim`: claims' lifecycle (list, heartbeat, hold)."""

import json
import os
import sys
import time
from datetime import datetime, timezone
from typing import Annotated, Optional

import typer

from ...boot.identity import agent_identity, session_identity
from ...boot.jsongit import hook_project
from ...boot.open import open_project
from ...common.core import format_duration, now, out, parse_iso, pid_alive, process_started, to_json
from ...common.errors import Invalid
from ...lib.claims import expire_claims, renew_due_claims, renew_lease
from ..app import app


# What the claim commands do beyond atm-lib: read a hook's input (`atm claim heartbeat --auto`, the plugin's
# PostToolUse hook), and hold a claim while a shell runs (`atm claim hold`, T-437).

# `atm claim hold`: how long a watcher may hold a claim, by default and at most
HOLD_MAX_MINUTES = 240.0
HOLD_LIMIT_MINUTES = 720.0


def hook_input():
    """The JSON a Claude Code hook gets on stdin ({session_id, cwd, ...}), or {}."""
    if sys.stdin is None or sys.stdin.isatty():
        return {}
    try:
        value = json.loads(sys.stdin.read() or "{}")
    except ValueError:
        return {}
    return value if isinstance(value, dict) else {}


def hold_claim(project, task_id, max_minutes, every=None):
    """`atm claim hold <id>`: renew the actor's claim now (refused, before anything else, when it isn't theirs), then
    leave a watcher behind that renews it while the shell that ran atm lives (atm's parent: the launchers exec), and
    return at once. The watcher stops when that process ends (or another one gets its pid: its start time differs),
    when a renewal fails (the claim was released, finished or taken over), or after `max_minutes`. It runs nothing:
    the agent runs its command itself, after this one. Returns {task, claim, watcher, watching, until}."""
    renewed = renew_lease(project, task_id)
    interval = every if every is not None else max(60.0, renewed["minutes"] * 60 / 3)
    shell = os.getppid()
    started = process_started(shell)
    deadline = time.time() + max_minutes * 60
    actor, cwd = project.actor, os.getcwd()

    def watch():
        while True:
            left = deadline - time.time()
            if left <= 0:
                return
            time.sleep(min(interval, left))
            if time.time() >= deadline or not pid_alive(shell) or process_started(shell) != started:
                return
            try:
                renew_lease(open_project(cwd, actor), task_id, expire=False)
            except Exception:  # released, finished, taken over, or the project is gone: the claim lapses as usual
                return

    until = datetime.fromtimestamp(deadline, timezone.utc).replace(microsecond=0)
    return {
        "task": task_id,
        "claim": renewed["claim"],
        "watcher": detach(watch),
        "watching": shell,
        "until": until.isoformat().replace("+00:00", "Z"),
    }


def detach(body):
    """Run `body` in a process of its own, detached from this one (a new session, stdin, stdout and stderr on
    /dev/null, so the caller's pipes close when atm exits), and return its pid."""
    read, write = os.pipe()
    child = os.fork()
    if child == 0:  # the intermediate: a new session, then the watcher, then gone (no zombie for the caller)
        os.close(read)
        os.setsid()
        watcher = os.fork()
        if watcher:
            os.write(write, str(watcher).encode())
            os._exit(0)
        os.close(write)
        null = os.open(os.devnull, os.O_RDWR)
        for fd in (0, 1, 2):
            os.dup2(null, fd)
        try:
            body()
        finally:
            os._exit(0)
    os.close(write)
    os.waitpid(child, 0)
    pid = int(os.read(read, 32) or 0)
    os.close(read)
    return pid


claim_app = typer.Typer(help="Claim lifecycle")
app.add_typer(claim_app, name="claim")


@claim_app.command("list")
def claim_list(
    ctx: typer.Context,
) -> None:
    """List live claims, shared by all worktrees of this clone."""
    live =expire_claims(open_project(os.getcwd()))["claims"]
    if ctx.obj.get("json_out", False):
        out(to_json(live))
        return
    if not live:
        out("No live claims.")
        return
    current = now()
    for c in live:
        left = format_duration((parse_iso(c["lease_expires_at"]) - current).total_seconds())
        out(f"{c['task']}  {c.get('agent')}  {c.get('branch') or '-'}  {left} left  ({c.get('worktree')})")


def _auto_heartbeat(agent):
    """`heartbeat --auto`, run by the plugin's PostToolUse hook after every tool call (CLI-6.2): renew this agent's
    live claims once their last renewal is ten minutes old. It never prints and never fails, so a tool call is never
    disturbed."""
    try:
        payload = hook_input()
        cwd = payload.get("cwd") if isinstance(payload.get("cwd"), str) and os.path.isdir(payload["cwd"]) else os.getcwd()
        if payload.get("session_id") and not (agent or os.environ.get("ATM_AGENT")):
            # The same identity atm commands get in that folder: a subagent in a linked worktree renews its own claims
            actor = session_identity(payload["session_id"], cwd)
        else:
            actor = agent_identity(agent)
        project = hook_project(cwd, actor)  # also lists the project on the dashboard
        if project is not None:
            renew_due_claims(project)
    except Exception:  # a hook must never break the tool call it follows
        return


@claim_app.command("heartbeat")
def claim_heartbeat(
    ctx: typer.Context,
    task_id: Optional[str] = typer.Argument(None, help="The task ID"),
    auto: bool = typer.Option(
        False, "--auto", help="Renew your claims if their last renewal is ten minutes old; silent (the plugin's hook)"
    ),
    agent: Optional[str] = typer.Option(None, "--agent", help="Who you are"),
) -> None:
    """Extend your claim's lease."""
    if ctx.obj.get("dry_run", False):
        out("[dry-run] would renew claims due for it" if auto else f"[dry-run] would extend the lease on {task_id}")
        return
    if auto:
        _auto_heartbeat(agent)
        return
    if not task_id:
        raise Invalid("atm claim heartbeat needs a task ID (or --auto)")
    claim = renew_lease(open_project(os.getcwd(), agent_identity(agent)), task_id)["claim"]
    if ctx.obj.get("json_out", False):
        out(to_json(claim))
    else:
        out(f"{task_id}: lease extended to {claim['lease_expires_at']}")


@claim_app.command("hold")
def claim_hold(
    ctx: typer.Context,
    task_id: Annotated[str, typer.Argument(help="The task ID")],
    max_minutes: float = typer.Option(
        HOLD_MAX_MINUTES, "--max", metavar="MINUTES", help=f"Stop after this long at most (up to {HOLD_LIMIT_MINUTES})"
    ),
    every: Optional[float] = typer.Option(
        None, "--every", metavar="SECONDS", help="Renew this often (default: three times per lease, at least once a minute)"
    ),
    agent: Optional[str] = typer.Option(None, "--agent", help="Who you are"),
) -> None:
    """Keep your claim alive while the shell that runs this lives: `atm claim hold T-1 && npm test`. atm runs nothing:
    a watcher renews the claim until that shell ends, the claim is released or finished, or --max is up."""
    if not 0 < max_minutes <= HOLD_LIMIT_MINUTES:
        raise Invalid(f"--max must be more than 0 and at most {HOLD_LIMIT_MINUTES} minutes")
    if every is not None and every <= 0:
        raise Invalid("--every must be more than 0 seconds")
    if ctx.obj.get("dry_run", False):
        out(f"[dry-run] would hold the claim on {task_id} while this shell runs")
        return
    held = hold_claim(open_project(os.getcwd(), agent_identity(agent)), task_id, max_minutes, every)
    if ctx.obj.get("json_out", False):
        out(to_json(held))
    else:
        out(
            f"{task_id}: holding the claim while this shell runs (pid {held['watching']}), until {held['until']} at "
            f"most; lease now to {held['claim']['lease_expires_at']}"
        )
