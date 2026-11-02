"""Claims: which agent works on which task, until when (the lease); and `next`.

Claims live in the git common directory, so all worktrees of a clone see them; tasks.json itself is per worktree.
Lock order: the task folder, then the claims store, so two commands can never wait on each other.
"""

import contextlib
import json
import os
import socket
import subprocess
import sys
from datetime import timedelta

from .core import (
    AtmError,
    AtmExit,
    format_duration,
    git,
    now,
    now_iso,
    out,
    parse_iso,
    read_json,
    to_json,
    write_json_atomic,
)
from .lock import locked
from .model import (
    REWORK_STATES,
    add_note,
    back_to_todo,
    change_status,
    find_sprint,
    kind_of,
    load_tasks,
    natural_key,
    priority_rank,
    unfinished_dependencies,
    vocabulary,
)
from .paths import find_paths, project_root
from .registry import main_worktree, register_if_needed

CLAIMS_STORE = "claims.json"  # in <git common dir>/atm/, shared by all worktrees of a clone
CLAIMS_STORE_NO_GIT = "claims-store.json"  # in the task folder, outside git
CLAIMS_SNAPSHOT = "claims.json"  # in the task folder (gitignored): what the dashboard reads
DEFAULT_LEASE_MINUTES = 30


def session_identity(session_id, cwd):
    """A Claude Code session's identity: "claude:" and 8 characters of its ID; in a linked git worktree, also
    "@<worktree folder>". Subagents share their parent's session ID, so agents spawned in separate worktrees still get
    identities of their own. The main worktree keeps the plain form, so its claims and history still match."""
    identity = "claude:" + str(session_id)[:8]
    root = project_root(cwd)
    if main_worktree(root) != root:
        identity += "@" + os.path.basename(root)
    return identity


def agent_identity(agent=None):
    """Who runs the command (the actor, DM-4.5): --agent, else ATM_AGENT, else this Claude Code session (per linked
    worktree, see session_identity), else the git user's name (a person in a terminal), else user@host."""
    if agent:
        return agent
    if os.environ.get("ATM_AGENT"):
        return os.environ["ATM_AGENT"]
    if os.environ.get("CLAUDE_CODE_SESSION_ID"):
        return session_identity(os.environ["CLAUDE_CODE_SESSION_ID"], os.getcwd())
    name = git(["config", "user.name"], project_root(os.getcwd()))
    if name:
        return name
    user = os.environ.get("USER") or os.environ.get("USERNAME") or "user"
    return f"{user}@{socket.gethostname()}"


def claims_dir(paths):
    common = git(["rev-parse", "--git-common-dir"], paths["root"])
    if common:
        return os.path.join(os.path.normpath(os.path.join(paths["root"], common)), "atm")
    return paths["dir"]


def claims_store_path(paths):
    directory = claims_dir(paths)
    return os.path.join(directory, CLAIMS_STORE if directory != paths["dir"] else CLAIMS_STORE_NO_GIT)


@contextlib.contextmanager
def locked_with_claims(paths):
    """Hold the task folder's lock and, if it is elsewhere, the claims store's lock (always in this order)."""
    with contextlib.ExitStack() as stack:
        stack.enter_context(locked(paths["dir"]))
        if claims_dir(paths) != paths["dir"]:
            stack.enter_context(locked(claims_dir(paths)))
        yield


def read_store(paths):
    """The shared store: live `claims`, and `completed` tasks (done in some worktree; its tasks.json may be the only
    one that says so until the branches merge, so `next` must not hand those tasks out again elsewhere); `waiting`
    tasks (blocked or sent for review in some worktree, likewise); `resolved` ones (answered, passed, rejected or
    skipped in another worktree than the one they wait in, to be applied there); and `hints`, when each hint was last
    shown (so `done` repeats one at most once a day)."""
    data = read_json(claims_store_path(paths))
    data = data if isinstance(data, dict) else {}

    def section(key):
        return data.get(key) if isinstance(data.get(key), dict) else {}

    return {
        "claims": data.get("claims") or {},
        "completed": data.get("completed") or {},
        "waiting": section("waiting"),
        "resolved": section("resolved"),
        "hints": section("hints"),
    }


def apply_resolutions(paths, data, store):
    """Apply what was decided in another worktree about a task waiting in this one (store["resolved"]): the answer,
    the passed or rejected review, the skip, as if given here, so this worktree's copy stops waiting too. A task changed
    here in the meantime is left as it is. Returns the IDs applied."""
    applied = []
    for task_id, record in list(store["resolved"].items()):
        if not isinstance(record, dict) or record.get("worktree") != paths["root"]:
            continue
        store["resolved"].pop(task_id)
        task = next((t for t in data["tasks"] if t.get("id") == task_id), None)
        if task is None:
            continue
        try:
            change_status(
                task,
                data,
                record.get("command"),
                agent=record.get("by"),
                reason=record.get("reason"),
                answers=record.get("answers") or [],
            )
            applied.append(task_id)
        except (AtmError, KeyError):
            pass
    return applied


def write_store(paths, store):
    os.makedirs(claims_dir(paths), exist_ok=True)
    write_json_atomic(claims_store_path(paths), store)
    write_claims_snapshot(paths, store["claims"])


def write_claims_snapshot(paths, claims):
    """The gitignored copy the dashboard polls: live claims, as a list."""
    if os.path.isdir(paths["dir"]):
        snapshot = {"generated_at": now_iso(), "claims": sorted(claims.values(), key=lambda c: c["task"])}
        write_json_atomic(os.path.join(paths["dir"], CLAIMS_SNAPSHOT), snapshot)


def lease_minutes(data):
    try:
        minutes = float(data.get("meta", {}).get("lease_minutes", DEFAULT_LEASE_MINUTES))
    except (TypeError, ValueError, AttributeError):
        return DEFAULT_LEASE_MINUTES
    return minutes if minutes > 0 else DEFAULT_LEASE_MINUTES


def lease_until(data):
    return (now() + timedelta(minutes=lease_minutes(data))).strftime("%Y-%m-%dT%H:%M:%SZ")


def expire_claims(paths, data, claims):
    """Drop claims whose lease ran out; their tasks in this worktree go back to todo. Returns the expired claims."""
    current = now()
    vocab = vocabulary(data)
    expired = []
    for task_id, claim in list(claims.items()):
        try:
            alive = parse_iso(claim["lease_expires_at"]) > current
        except (KeyError, TypeError, ValueError):
            alive = False
        if alive:
            continue
        expired.append(claims.pop(task_id))
        task = next((t for t in data["tasks"] if t.get("id") == task_id), None)
        if task and kind_of(vocab, task.get("status")) == "active":
            stamp = now_iso()
            back_to_todo(task, vocab, None, stamp, keep_commit=True)
            add_note(task, "failure", f"lease expired ({claim.get('agent')})", None, stamp)
    return expired


def check_holder(claims, task_id, agent, command):
    claim = claims.get(task_id)
    if claim and claim.get("agent") != agent:
        raise AtmError(
            f"can't {command} {task_id}: it is claimed by {claim.get('agent')} until {claim.get('lease_expires_at')}",
            4,
        )


def own_lease_expired(expired, data, task_id, agent, then):
    """When `agent`'s own claim on `task_id` is among the claims that just expired and the task went back to todo: an
    error that says so and how to go on (`then`, the command to run after `atm start`). Exit code 4, like a claim
    conflict. Another agent's expired claim, or none: None."""
    claim = next((c for c in expired if c.get("task") == task_id and c.get("agent") == agent), None)
    task = next((t for t in data["tasks"] if t.get("id") == task_id), None)
    if not claim or not task or kind_of(vocabulary(data), task.get("status")) != "todo":
        return None
    until = str(claim.get("lease_expires_at", ""))
    at = f"{until[11:16]} UTC" if len(until) >= 16 else until
    minutes = lease_minutes(data)
    idle = f"{minutes:g} min"
    kept = " Your commit is kept." if task.get("commit") else ""
    next_steps = f"Run `atm start {task_id}`" + (f", then `{then}` again." if then else " to claim it again.")
    return AtmError(
        f"{task_id}: your lease expired at {at} (no heartbeat for {idle}), so it went back to todo.{kept} {next_steps}",
        4,
    )


def report_expired(expired):
    for claim in expired:
        sys.stderr.write(f"atm: lease of {claim['task']} by {claim.get('agent')} expired; it is available again\n")


def ready_in_order(data, store, candidates=None, rank=None):
    """The to-do tasks the loop could take, in the order it takes them: every dependency done, not claimed, not
    completed or waiting in another worktree; by `rank` (the selection's own order, SR-3.2), then the most urgent
    priority, then lowest ID."""
    vocab = vocabulary(data)
    if candidates is None:
        candidates = [t for t in data["tasks"] if kind_of(vocab, t.get("status")) == "todo"]
    taken = set(store["claims"]) | set(store["completed"]) | set(store["waiting"])
    ready = [t for t in candidates if not unfinished_dependencies(data, t) and t.get("id") not in taken]
    rank = rank or (lambda t: 0)
    order = lambda t: (rank(t), priority_rank(vocab, t.get("priority")), natural_key(t.get("id")))
    return sorted(ready, key=order)


class Selection:
    """What the loop works on (docs/sprints-and-releases-design.md §3.2): the active sprint (the default in a project
    with sprints), named sprints, a list of tasks, all tasks, or the backlog; `task_type` narrows it."""

    def __init__(self, sprints=(), tasks=(), all_tasks=False, backlog=False, task_type=None):
        self.sprints, self.tasks, self.all, self.backlog = list(sprints or []), list(tasks or []), all_tasks, backlog
        self.task_type = task_type


def select(data, sel):
    """The tasks `sel` covers, in any status, with how to rank them and a word for them. Returns (tasks, rank, scope,
    reason): reason is set when the selection can't be made (no active sprint, a closed sprint)."""
    vocab = vocabulary(data)
    sprints = vocab["sprints"]
    in_sprint = lambda names: (lambda t: (find_sprint(vocab, t.get("sprint")) or {}).get("id") in names)
    rank, scope, reason = None, "the tasks", None
    if sel.tasks:
        order = {task_id: i for i, task_id in enumerate(sel.tasks)}
        missing = [i for i in sel.tasks if i not in {t.get("id") for t in data["tasks"]}]
        if missing:
            raise AtmError(f"--tasks: no task {', '.join(missing)}", 2)
        keep, rank, scope = (lambda t: t.get("id") in order), (lambda t: order[t.get("id")]), "the listed tasks"
    elif sel.all:
        # The active sprint, then the planned ones in order, then the backlog (closed sprints' leftovers before it)
        place = {
            s["id"]: 0 if s["state"] == "active" else 1 + i if s["state"] == "planned" else len(sprints) + 1
            for i, s in enumerate(sprints)
        }
        keep = lambda t: True
        rank = lambda t: place.get((find_sprint(vocab, t.get("sprint")) or {}).get("id"), len(sprints) + 2)
        scope = "all tasks"
    elif sel.backlog:
        keep, scope = (lambda t: not find_sprint(vocab, t.get("sprint"))), "the backlog"
    elif sel.sprints:
        named = []
        for name in sel.sprints:
            sprint = find_sprint(vocab, name)
            if sprint is None:
                raise AtmError(f"no sprint {name!r}; the sprints: {', '.join(s['id'] for s in sprints) or 'none'}", 2)
            if sprint["state"] == "closed":
                raise AtmError(f"the sprint {sprint['id']!r} is closed", 2)
            named.append(sprint["id"])
        keep, scope = in_sprint(named), "the sprint " + " and ".join(f"“{n}”" for n in named)
    elif sprints:
        active = [s["id"] for s in sprints if s["state"] == "active"]
        if not active:
            return [], None, "the active sprint", "no active sprint: plan one with /atm:plan-sprint, or run with --all"
        keep, scope = in_sprint(active), f"the sprint “{active[0]}”"
    else:
        keep = lambda t: True  # a project without sprints: every task, as before sprints
    tasks = [
        t
        for t in data["tasks"]
        if keep(t)
        and (sel.task_type is None or t.get("type") == sel.task_type)
    ]
    return tasks, rank, scope, reason


def offered_to(data, task, agent):
    """Whether a task back from review (rework or approved) may go to `agent`: always to the agent that worked on it;
    to others once one lease length has passed since the review ended (SR-3.9, Q11)."""
    if not agent or task.get("started_by") == agent:
        return True
    try:
        reviewed = parse_iso(task.get("reviewed_at"))
    except (TypeError, ValueError):
        return True
    return now() >= reviewed + timedelta(minutes=lease_minutes(data))


def loop_order(data, store, sel, agent=None, with_user=False):
    """Every task the loop could take now, in the order it takes them: `atm next` hands out the first, and atm serve's
    summary lists them as `next`, which the dashboard's Next up shows (T-321), so the two can't disagree. Returns
    (ready, selected, candidates, together, scope, problem); `problem` says why the selection itself is refused."""
    vocab = vocabulary(data)
    selected, rank, scope, problem = select(data, sel)
    if problem:
        return [], selected, [], [], scope, problem
    # Tasks back from review come first (SR-3.9): the agent that worked on them gets them back for a lease length
    rework = [
        t
        for t in selected
        if kind_of(vocab, t.get("status")) == "review"
        and t.get("review_state") in REWORK_STATES
        and offered_to(data, t, agent)
    ]
    first = {t.get("id") for t in rework}
    candidates = rework + [t for t in selected if kind_of(vocab, t.get("status")) == "todo"]
    # Work done together with the user waits for them, unless they're watching (SR-3.5)
    together = [t for t in candidates if t.get("with") == "user"]
    if not with_user:
        candidates = [t for t in candidates if t.get("with") != "user"]
    in_order = rank or (lambda t: 0)
    ready = ready_in_order(data, store, candidates, lambda t: (t.get("id") not in first, in_order(t)))
    return ready, selected, candidates, together, scope, None


def next_task(data, store, task_type=None, sel=None, agent=None, with_user=False):
    """The loop's rule (same as the dashboard's "Next up"): todo (or back from review), every dependency done, not
    claimed and not completed in another worktree, in the selection (the default one when `sel` is None); in its
    order, then the most urgent priority, then lowest ID. Returns (task, reason) where reason explains an empty
    result: the selection is finished, the rest of it waits, or it is empty (SR-3.6)."""
    vocab = vocabulary(data)
    sel = sel or Selection(task_type=task_type)
    ready, selected, candidates, together, scope, problem = loop_order(data, store, sel, agent, with_user)
    if problem:
        return None, problem
    if ready:
        return ready[0], None
    if together and not with_user and not candidates:
        ids = ", ".join(t.get("id") for t in together)
        return None, f"only work done together with the user is left in {scope}: {ids} (--with-user takes it)"
    if not candidates:
        if not selected:
            if not vocab["sprints"]:
                return None, "nothing left to do"
            return None, f"{scope} {'has' if scope.startswith('the sprint') else 'have'} no tasks"
        kinds = [kind_of(vocab, t.get("status")) for t in selected]
        open_kinds = {k: kinds.count(k) for k in ("active", "review", "blocked") if k in kinds}
        if open_kinds:
            words = {"active": "in progress", "review": "in review", "blocked": "blocked"}
            what = ", ".join(f"{n} {words[k]}" for k, n in open_kinds.items())
            return None, f"nothing ready in {scope}: the rest is waiting ({what})"
        if vocab["sprints"]:
            return None, f"{scope} is finished: {kinds.count('done')} done" + (
                f", {kinds.count('skipped')} skipped" if kinds.count("skipped") else ""
            )
        return None, "nothing left to do"
    claimed = [t["id"] for t in candidates if t.get("id") in store["claims"]]
    done_elsewhere = [t["id"] for t in candidates if t.get("id") in store["completed"]]
    waiting_elsewhere = [t["id"] for t in candidates if t.get("id") in store["waiting"]]
    reason = f"{len(candidates)} todo task(s) waiting on dependencies"
    if claimed:
        reason += f" or claimed by other agents ({', '.join(claimed)})"
    if done_elsewhere:
        reason += f"; done in another worktree, not merged yet: {', '.join(done_elsewhere)}"
    if waiting_elsewhere:
        reason += f"; blocked or in review in another worktree: {', '.join(waiting_elsewhere)}"
    return None, reason


