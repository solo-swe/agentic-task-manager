"""atm-lib: the loop's rules (docs/parallel-agents-design.md; docs/sprints-and-releases-design.md §3): leases, who
holds a claim, what was decided in another worktree, and which task the loop takes next (`next_task`, the same order
the dashboard's Next up shows). They work on the task data and the claims store as the store hands them out
(`work.document`, `work.claims_store`), with the project's lookup of archived tasks (`archived`: a dependency only
in the archive is done); keeping the claims is the store's (db/jsongit/claims.py)."""

from datetime import timedelta

from ..common.core import AtmError, now, now_iso, parse_iso
from ..common.model import (
    REWORK_STATES,
    add_note,
    back_to_todo,
    find_sprint,
    kind_of,
    natural_key,
    priority_rank,
    unfinished_dependencies,
    vocabulary,
)
from .state import change_status

DEFAULT_LEASE_MINUTES = 30


def archived_in(work):
    """The lookup of archived tasks inside a store change (`archived(task_id)` -> task or None): the change's own
    archive, read only when something asks for it."""
    index = {}

    def lookup(task_id):
        if not index:
            index.update({t.get("id"): t for t in work.archive()["tasks"] if isinstance(t, dict)} or {None: None})
        return index.get(task_id)

    return lookup


def apply_resolutions(worktree, data, store, archived):
    """Apply what was decided in another worktree about a task waiting in this one (store["resolved"]): the answer,
    the passed or rejected review, the skip, as if given here, so this worktree's copy stops waiting too. `worktree` is
    this working copy's folder, `archived` the project's lookup of archived tasks. A task changed here in the meantime
    is left as it is. Returns the IDs applied."""
    applied = []
    for task_id, record in list(store["resolved"].items()):
        if not isinstance(record, dict) or record.get("worktree") != worktree:
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
                archived=archived,
            )
            applied.append(task_id)
        except (AtmError, KeyError):
            pass
    return applied


def lease_minutes(data):
    try:
        minutes = float(data.get("meta", {}).get("lease_minutes", DEFAULT_LEASE_MINUTES))
    except (TypeError, ValueError, AttributeError):
        return DEFAULT_LEASE_MINUTES
    return minutes if minutes > 0 else DEFAULT_LEASE_MINUTES


def lease_until(data):
    return (now() + timedelta(minutes=lease_minutes(data))).strftime("%Y-%m-%dT%H:%M:%SZ")


def drop_expired_claims(data, claims):
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


def ready_in_order(data, store, candidates=None, rank=None, *, archived):
    """The to-do tasks the loop could take, in the order it takes them: every dependency done (`archived`: the
    project's lookup of archived tasks, which are), not claimed, not completed or waiting in another worktree; by
    `rank` (the selection's own order, SR-3.2), then the most urgent priority, then lowest ID."""
    vocab = vocabulary(data)
    if candidates is None:
        candidates = [t for t in data["tasks"] if kind_of(vocab, t.get("status")) == "todo"]
    taken = set(store["claims"]) | set(store["completed"]) | set(store["waiting"])
    ready = [t for t in candidates if not unfinished_dependencies(data, t, archived) and t.get("id") not in taken]
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


def loop_order(data, store, sel, agent=None, with_user=False, *, archived):
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
    ready = ready_in_order(
        data, store, candidates, lambda t: (t.get("id") not in first, in_order(t)), archived=archived
    )
    return ready, selected, candidates, together, scope, None


def next_task(data, store, task_type=None, sel=None, agent=None, with_user=False, *, archived):
    """The loop's rule (same as the dashboard's "Next up"): todo (or back from review), every dependency done, not
    claimed and not completed in another worktree, in the selection (the default one when `sel` is None); in its
    order, then the most urgent priority, then lowest ID. Returns (task, reason) where reason explains an empty
    result: the selection is finished, the rest of it waits, or it is empty (SR-3.6)."""
    vocab = vocabulary(data)
    sel = sel or Selection(task_type=task_type)
    ready, selected, candidates, together, scope, problem = loop_order(
        data, store, sel, agent, with_user, archived=archived
    )
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
