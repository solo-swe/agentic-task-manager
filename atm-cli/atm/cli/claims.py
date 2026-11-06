"""What the claim commands do beyond atm-lib: read a hook's input (`atm claim heartbeat --auto`, the plugin's
PostToolUse hook), and hold a claim while a shell runs (`atm claim hold`, T-437)."""

import json
import os
import sys
import time
from datetime import datetime, timezone

from ..boot.open import open_project
from ..common.core import pid_alive, process_started
from ..lib.claims import renew_lease

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
