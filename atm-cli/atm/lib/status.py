"""atm-lib: the status changes (start, review, done, reject, approve, request-changes, block, unblock, reopen, skip,
fail, release) and notes (docs/layered-architecture.md §5.1).

Each runs as one change of the store (`store.change`, §3.3): the task's status, its claim, and the records shared by
a clone's worktrees (completed, waiting, resolved) change together or not at all. The transitions themselves are
common's (`change_status`, common/model.py), the lease and resolution rules lib/loop.py's.
"""

from ..common.errors import Invalid
from ..common.core import now_iso
from ..common.model import (
    add_note,
    archive_hint_for,
    find_sprint,
    find_task,
    kind_of,
    move_out_hint,
    touch,
    vocabulary,
)
from .loop import apply_resolutions, archived_in, check_holder, drop_expired_claims, lease_until, own_lease_expired
from .query import Filters, ready_tasks, task_views
from .state import change_status
from .tasks import show_tasks, view_task

# The commands an agent runs on its own claimed task: if that claim just expired, they explain it instead of refusing
OWN_LEASE_COMMANDS = ("done", "release", "fail", "review", "block")

# The state `atm task state <id> <state>` names for the commands an agent runs on its own claim, for the hint that
# says what to run again
COMMAND_STATES = {"done": "done", "release": "todo", "fail": "todo", "review": "in_review", "block": "blocked"}

# Decisions that may be made in any worktree about a task waiting in another (T-204): the command, and the kinds the
# waiting task must have there
REMOTE_DECISIONS = {"unblock": ("blocked",), "skip": ("blocked",), "done": ("review",), "reject": ("review",)}


def todo_command(project, task_id, failing=False):
    """The status function `atm task state <id> todo` runs, by the kind the task is in: in progress, a failure
    (`failing`, it says why) or a release; in review, reject; blocked, unblock; done or skipped, reopen. A task that is
    to do here but claimed (in progress in another worktree, T-202), or blocked or in review in another worktree
    (T-204), is decided as it is there. Raises Invalid when it is to do already."""
    view = next((v for v in task_views(project, Filters()) if v["task"].get("id") == task_id), None)
    if view is None:
        show_tasks(project, [task_id])  # NotFound when it doesn't exist; otherwise it is archived (finished)
        kind = "done"
    else:
        kind = view["derived"].get("kind")
    if kind == "todo":
        store = project.store.read_claims()
        record = store["waiting"].get(task_id)
        if task_id in store["claims"]:
            kind = "active"  # the claim's holder check says whose it is
        elif isinstance(record, dict) and record.get("worktree") != project.store.working_copy():
            kind = kind_of(vocabulary({"meta": project.store.get_settings()}), record.get("status"))
    commands = {
        "active": "fail" if failing else "release",
        "review": "reject",
        "blocked": "unblock",
        "done": "reopen",
        "skipped": "reopen",
    }
    if kind not in commands:
        raise Invalid(f"{task_id} is already {kind}")
    return commands[kind]


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


def newly_ready(data, task_id, archived):
    """The to-do tasks waiting on `task_id` whose dependencies are now all done."""
    ready = {t.get("id") for t in ready_tasks(data, archived)}
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
        data, store, archived = work.document, work.claims(), archived_in(work)
        claims = store["claims"]
        here = work.here()["worktree"]
        expired = drop_expired_claims(data, claims)
        apply_resolutions(here, data, store, archived)
        task = find_task(data, task_id, archived)
        if command in OWN_LEASE_COMMANDS:
            own = own_lease_expired(expired, data, task_id, agent, f"atm task state {task_id} {COMMAND_STATES[command]}")
            if own:
                work.defer(own)  # the expiry is written first, so `atm task details` agrees with the error
                return {"task": task, "expired": expired}
        check_holder(claims, task_id, agent, command)
        duplicate_of = options.get("duplicate_of")
        if duplicate_of and not any(t.get("id") == duplicate_of for t in data["tasks"]) and not archived(duplicate_of):
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
            archived=archived,
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
        unblocked = newly_ready(data, task_id, archived) if command == "done" else []
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
        from_archive = all(t is not task for t in data["tasks"])
        return {
            "task": task,
            "previous": previous,
            "claim": claims.get(task_id),
            "unblocked": unblocked,
            "hint": hint,
            "stranded": stranded,
            "expired": expired,
            # The task as `atm show` gives it (show_tasks), so a front end can show it without reading the project
            "view": view_task(task, from_archive, data["tasks"], archived, vocabulary(data)),
        }

    return project.store.change(change)


# One function per status change (§5.1, T-383), each with the parameters its command takes. `status` picks the status
# when the project has several of the target kind. Each returns a StatusChange (api.schema.json): the task after it,
# its status before (`previous`), its claim now, the tasks now ready (`unblocked`, after done), the archive hint (at
# most once a day), `stranded` (reopened in a closed sprint) and the claims that `expired` on the way.


def start_task(project, task_id, status=None):
    """To do -> in progress, claimed by `project.actor`; its dependencies must be done."""
    return change_task_status(project, "start", task_id, status=status)


def review_task(project, task_id, note=None, status=None):
    """Asks for a review (`note`: what the reviewer should check); the claim is released."""
    return change_task_status(project, "review", task_id, note=note, status=status)


def done_task(project, task_id, note=None, commit=None, resolution=None, status=None):
    """Done (or waiting for the sprint review, by its review mode); the claim dropped."""
    return change_task_status(project, "done", task_id, note=note, commit=commit, resolution=resolution, status=status)


def reject_task(project, task_id, reason, status=None):
    """A task in review back to to do, for anyone."""
    return change_task_status(project, "reject", task_id, reason=reason, status=status)


def approve_task(project, task_id, note=None, status=None):
    """Approves a review; it stays in review until its agent takes it back."""
    return change_task_status(project, "approve", task_id, note=note, status=status)


def request_changes(project, task_id, reason, status=None):
    """Asks for changes; its agent gets it back first."""
    return change_task_status(project, "request-changes", task_id, reason=reason, status=status)


def block_task(project, task_id, on, questions, reason=None, status=None):
    """Blocked on a decision by `on` (user or agent), with its questions; the claim released."""
    return change_task_status(project, "block", task_id, on=on, questions=questions, reason=reason, status=status)


def unblock_task(project, task_id, answers, status=None):
    """Back to to do, with the answers (one per question) as a note."""
    return change_task_status(project, "unblock", task_id, answers=answers, status=status)


def reopen_task(project, task_id, reason, status=None):
    """A done or skipped task back to to do."""
    return change_task_status(project, "reopen", task_id, reason=reason, status=status)


def skip_task(project, task_id, reason, resolution=None, duplicate_of=None, status=None):
    """Not needed: skipped (a to-do or blocked task)."""
    return change_task_status(
        project, "skip", task_id, reason=reason, resolution=resolution, duplicate_of=duplicate_of, status=status
    )


def fail_task(project, task_id, reason=None, status=None):
    """In progress back to to do, with a failure note."""
    return change_task_status(project, "fail", task_id, reason=reason, status=status)


def release_task(project, task_id, note=None, status=None):
    """In progress back to to do, with a handoff note."""
    return change_task_status(project, "release", task_id, note=note, status=status)


# Each command's function, for a front end that takes the command's name (the CLI)
STATUS_FUNCTIONS = {
    "start": start_task,
    "review": review_task,
    "done": done_task,
    "reject": reject_task,
    "approve": approve_task,
    "request-changes": request_changes,
    "block": block_task,
    "unblock": unblock_task,
    "reopen": reopen_task,
    "skip": skip_task,
    "fail": fail_task,
    "release": release_task,
}


def add_task_note(project, task_id, text):
    """A note on a task, whatever its status (DM-4), at most 500 characters. Returns the task."""

    def change(work):
        task = find_task(work.document, task_id, archived_in(work))
        stamp = now_iso()
        add_note(task, "note", text, project.actor, stamp)
        touch(task, project.actor, stamp)
        return task

    return project.store.change(change)
