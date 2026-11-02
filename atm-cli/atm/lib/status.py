"""atm-lib: the status changes (start, review, done, reject, approve, request-changes, block, unblock, reopen, skip,
fail, release) and notes (docs/layered-architecture.md §5.1).

Each runs as one change of the store (`store.change`, §3.3): the task's status, its claim, and the records shared by
a clone's worktrees (completed, waiting, resolved) change together or not at all. The rules themselves are still
those of the modules from before the layers (`change_status`, the lease and resolution helpers); they move into
atm-lib with their own slices (§9).
"""

from ..claims import apply_resolutions, check_holder, expire_claims, lease_until, own_lease_expired
from ..common.errors import Invalid
from ..core import now_iso
from ..model import (
    add_note,
    archive_hint_for,
    archived_task,
    change_status,
    find_sprint,
    find_task,
    kind_of,
    move_out_hint,
    touch,
    vocabulary,
)
from .query import ready_tasks

# The commands an agent runs on its own claimed task: if that claim just expired, they explain it instead of refusing
OWN_LEASE_COMMANDS = ("done", "release", "fail", "review", "block")

# Decisions that may be made in any worktree about a task waiting in another (T-204): the command, and the kinds the
# waiting task must have there
REMOTE_DECISIONS = {"unblock": ("blocked",), "skip": ("blocked",), "done": ("review",), "reject": ("review",)}


def claim_record(work, data, task_id, agent):
    """A new claim on the task, from this working copy (its worktree and branch, T-202)."""
    here = work.here()
    return {
        "task": task_id,
        "agent": agent,
        "branch": here["branch"],
        "worktree": here["worktree"],
        "claimed_at": now_iso(),
        "lease_expires_at": lease_until(data),
    }


def completion_record(work, task_id, agent):
    """Done in this working copy: so `next` doesn't hand the task out again in another before the branches merge."""
    here = work.here()
    return {"task": task_id, "agent": agent, "branch": here["branch"], "worktree": here["worktree"], "completed_at": now_iso()}


def waiting_entry(work, task):
    """The shared record of a task blocked or sent for review in this working copy (T-204)."""
    here = work.here()
    return {
        "task": task["id"],
        "status": task.get("status"),
        "waiting_on": task.get("waiting_on"),
        "questions": list(task.get("questions") or []),
        "worktree": here["worktree"],
        "branch": here["branch"],
        "at": now_iso(),
    }


def adopt_waiting(task, data, store, here, command):
    """When `task` waits in another worktree (blocked or in review there, per the shared store) but not in this one,
    and `command` decides it: give this copy the waiting state, so the command applies here as it would there. Returns
    the worktree it waits in (where the decision is applied too, by apply_resolutions), or None."""
    record = store["waiting"].get(task["id"])
    if command not in REMOTE_DECISIONS or not isinstance(record, dict) or record.get("worktree") == here:
        return None
    vocab = vocabulary(data)
    if kind_of(vocab, task.get("status")) in REMOTE_DECISIONS[command]:
        return None  # it waits here too: decided here as usual
    if kind_of(vocab, record.get("status")) not in REMOTE_DECISIONS[command]:
        return None
    task["status"] = record["status"]
    if record.get("waiting_on"):
        task["waiting_on"] = record["waiting_on"]
    if record.get("questions"):
        task["questions"] = list(record["questions"])
    return record.get("worktree")


def newly_ready(data, task_id):
    """The to-do tasks waiting on `task_id` whose dependencies are now all done."""
    ready = {t.get("id") for t in ready_tasks(data)}
    return [t.get("id") for t in data["tasks"] if task_id in (t.get("dependencies") or []) and t.get("id") in ready]


def change_task_status(project, command, task_id, **options):
    """One status command on a task, as `project.actor`. `options` are the command's: status, note, reason, commit,
    on, questions, answers, resolution, duplicate_of. Returns the task after the change and what goes with it:
    `previous` (its status before), `claim` (its claim now), `unblocked` (after done), `hint` (the archive hint, at
    most once a day), `stranded` (reopened in a closed sprint, T-330) and `expired` (the claims whose lease ran out)."""
    agent = project.actor
    status = options.get("status")
    answers = list(options.get("answers") or [])

    def change(work):
        data, store = work.document, work.claims()
        claims = store["claims"]
        here = work.here()["worktree"]
        expired = expire_claims(None, data, claims)
        # apply_resolutions knows a working copy by its folder, as atm-cli's paths give it
        apply_resolutions({"root": here}, data, store)
        task = find_task(data, task_id)
        if command in OWN_LEASE_COMMANDS:
            own = own_lease_expired(expired, data, task_id, agent, f"atm {command} {task_id}")
            if own:
                work.defer(own)  # the expiry is written first, so `atm show` agrees with the error
                return {"task": task, "expired": expired}
        check_holder(claims, task_id, agent, command)
        duplicate_of = options.get("duplicate_of")
        if duplicate_of and not any(t.get("id") == duplicate_of for t in data["tasks"]) and not archived_task(duplicate_of):
            raise Invalid(f"{task_id} can't be a duplicate of {duplicate_of}, which doesn't exist")
        waits_in = adopt_waiting(task, data, store, here, command)
        previous = change_status(
            task,
            data,
            command,
            reason=options.get("reason"),
            agent=agent,
            status=status,
            note=options.get("note"),
            commit=options.get("commit"),
            waiting_on=options.get("on"),
            questions=list(options.get("questions") or []),
            answers=answers,
            resolution=options.get("resolution"),
            duplicate_of=duplicate_of,
        )
        if command == "start":
            claims[task_id] = claim_record(work, data, task_id, agent)
            store["completed"].pop(task_id, None)  # starting a task again on purpose
        elif command == "reopen":
            claims.pop(task_id, None)
            store["completed"].pop(task_id, None)  # not done any more, in any worktree
        else:
            claims.pop(task_id, None)
        if command == "done":
            store["completed"][task_id] = completion_record(work, task_id, agent)
        # Blocked or in review: recorded for every worktree (T-204); any other change ends the wait
        if command in ("block", "review"):
            store["waiting"][task_id] = waiting_entry(work, task)
        else:
            store["waiting"].pop(task_id, None)
            resolution = store["resolved"].pop(task_id, None)
            if waits_in:
                store["resolved"][task_id] = {
                    "task": task_id,
                    "command": command,
                    "answers": answers,
                    "reason": options.get("reason"),
                    "by": agent,
                    "at": now_iso(),
                    "worktree": waits_in,
                }
            elif resolution and resolution.get("worktree") != here:
                store["resolved"][task_id] = resolution  # still to be applied where it waits
        unblocked = newly_ready(data, task_id) if command == "done" else []
        # The archive hint after done, at most once a day per clone (CLI-5.2), so a loop doesn't repeat it
        hint = archive_hint_for(data, work.size()) if command == "done" else None
        today = now_iso()[:10]
        if hint and store["hints"].get("archive") == today:
            hint = None
        if hint:
            store["hints"]["archive"] = today
        # Reopened in a closed sprint, the task is open where the loop's default selection never looks (T-330)
        stranded = None
        if command == "reopen":
            sprint = find_sprint(vocabulary(data), task.get("sprint"))
            if sprint and sprint["state"] == "closed":
                stranded = f"{task['id']} is now open in {move_out_hint(task['id'], sprint['id'])}"
        return {
            "task": task,
            "previous": previous,
            "claim": claims.get(task_id),
            "unblocked": unblocked,
            "hint": hint,
            "stranded": stranded,
            "expired": expired,
            "document": data,
        }

    return project.store.change(change)


def add_task_note(project, task_id, text):
    """A note on a task, whatever its status (DM-4), at most 500 characters. Returns the task."""

    def change(work):
        task = find_task(work.document, task_id)
        stamp = now_iso()
        add_note(task, "note", text, project.actor, stamp)
        touch(task, project.actor, stamp)
        return task

    return project.store.change(change)
