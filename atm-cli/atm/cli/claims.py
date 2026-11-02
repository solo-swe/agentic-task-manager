"""`atm next`, `heartbeat`, `run` and `claims`, through atm-lib (docs/layered-architecture.md §9, step 9)."""

import json
import os
import subprocess
import sys

from ..boot.open import open_project
from ..claims import agent_identity, report_expired, session_identity
from ..common.errors import AtmFailure, Invalid
from ..common.core import AtmExit, format_duration, now, out, parse_iso, to_json
from ..lib.claims import live_claims, next_task_for, renew_due_claims, renew_lease, selection
from ..paths import find_paths
from ..registry import register_if_needed


def cmd_next(args):
    sel = selection(
        sprints=args.sprint,
        tasks=[i.strip() for i in (args.tasks or "").split(",") if i.strip()],
        all_tasks=args.all,
        backlog=args.backlog,
        task_type=args.type,
    )
    project = open_project(os.getcwd(), agent_identity(args.agent))
    result = next_task_for(project, sel, with_user=args.with_user, claim=args.claim, status=args.status)
    report_expired(result["expired"])
    task, claim, reason = result["task"], result["claim"], result["reason"]
    if args.json:
        out(to_json({"task": task, "claim": claim, "reason": reason}))
    elif task:
        verb = "Claimed" if claim else "Next"
        where = task.get("sprint") or "backlog"
        out(f"{verb}: {task['id']} {task.get('name', '')} ({where}, {task.get('type')})")
    else:
        out(f"No task available: {reason}.")
    if not task:
        raise AtmExit(3)


def hook_input():
    """The JSON a Claude Code hook gets on stdin ({session_id, cwd, ...}), or {}."""
    if sys.stdin is None or sys.stdin.isatty():
        return {}
    try:
        value = json.loads(sys.stdin.read() or "{}")
    except ValueError:
        return {}
    return value if isinstance(value, dict) else {}


def auto_heartbeat(args):
    """`heartbeat --auto`, run by the plugin's PostToolUse hook after every tool call (CLI-6.2): renew this agent's
    live claims once their last renewal is ten minutes old. It never prints and never fails, so a tool call is never
    disturbed."""
    try:
        payload = hook_input()
        cwd = payload.get("cwd") if isinstance(payload.get("cwd"), str) and os.path.isdir(payload["cwd"]) else os.getcwd()
        if payload.get("session_id") and not (args.agent or os.environ.get("ATM_AGENT")):
            # The same identity atm commands get in that folder: a subagent in a linked worktree renews its own claims
            agent = session_identity(payload["session_id"], cwd)
        else:
            agent = agent_identity(args.agent)
        paths = find_paths(cwd)
        if not paths["tasks_exists"]:
            return
        try:
            register_if_needed(paths)  # list the project on the dashboard (one read of the registry when it is)
        except Exception:
            pass
        renew_due_claims(open_project(cwd, agent))
    except Exception:  # a hook must never break the tool call it follows
        return


def cmd_heartbeat(args):
    if args.auto:
        auto_heartbeat(args)
        return
    if not args.id:
        raise Invalid("atm heartbeat needs a task ID (or --auto)")
    result = renew_lease(open_project(os.getcwd(), agent_identity(args.agent)), args.id)
    claim = result["claim"]
    if args.json:
        out(to_json(claim))
    else:
        out(f"{args.id}: lease extended to {claim['lease_expires_at']}")


def cmd_run(args):
    """`atm run <id> -- <command>`: run a long command (a full test suite, a build, a wait on CI) while keeping the
    claim on <id> alive. The plugin's hook renews claims only when the agent makes a tool call, and an agent waiting
    for a command makes none, so a wait longer than the lease would hand the task to someone else (T-203). The claim
    is renewed at the start (so a wrong ID or someone else's claim fails before anything runs) and then three times per
    lease; the command's output passes through unchanged, and atm exits with its exit code."""
    if not args.child:
        raise Invalid("atm run needs a command after --: atm run <id> -- <command> [args...]")
    if args.every is not None and args.every <= 0:
        raise Invalid("--every must be more than 0 seconds")
    project = open_project(os.getcwd(), agent_identity(args.agent))
    renew = lambda: renew_lease(open_project(os.getcwd(), project.actor), args.id, expire=False)
    minutes = renew()["minutes"]
    every = args.every if args.every is not None else max(60.0, minutes * 60 / 3)
    try:
        child = subprocess.Popen(args.child)
    except OSError as e:
        raise AtmFailure(f"can't run {args.child[0]}: {e.strerror}", 127)
    # Renewed from this (the main) thread between waits: the lock uses signals, which only work here
    renewing = True
    while True:
        try:
            code = child.wait(timeout=every)
            break
        except subprocess.TimeoutExpired:
            pass
        except KeyboardInterrupt:  # the command got the same interrupt; wait for it to end
            continue
        if renewing:
            try:
                renew()
            except Exception as e:  # never stops the command; say so once and stop renewing
                sys.stderr.write(f"atm run: stopped renewing the claim on {args.id}: {e}\n")
                renewing = False
    if code:
        raise AtmExit(128 - code if code < 0 else code)  # a signal -N ends like a shell's 128+N


def cmd_claims(args):
    result = live_claims(open_project(os.getcwd()))
    live = result["claims"]
    if args.json:
        out(to_json(live))
        return
    if not live:
        out("No live claims.")
        return
    current = now()
    for c in live:
        left = format_duration((parse_iso(c["lease_expires_at"]) - current).total_seconds())
        out(f"{c['task']}  {c.get('agent')}  {c.get('branch') or '-'}  {left} left  ({c.get('worktree')})")
