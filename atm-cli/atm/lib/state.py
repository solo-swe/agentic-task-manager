"""Task state machine using the transitions library.

Declares the task status transitions as data, with their guards (conditions).
Replaces the hand-rolled change_status() that was in common/model.py, and
refuses and records exactly as it did: the same messages, exit codes, notes
and key order in tasks.json (the scenarios in tests/scripts/ check them).

The machine state is always a *kind* (todo, active, review, blocked, done,
skipped), never a specific status name. The task's `status` field holds the
project-specific status name (e.g. `in_progress` for kind `active`), mapped
from the kind with status_for().

Order of checks (as before): the resolution, then whether the command applies
to the task's kind, then the guards, then the notes and the block's arguments.
Only then does anything change, in one place (_apply).
"""

from __future__ import annotations

from transitions import Machine

from ..common.core import AtmError, now_iso
from ..common.model import (
    RESOLUTIONS,
    REWORK_STATES,
    SKIP_RESOLUTIONS,
    WAITING_ON,
    add_interval,
    add_note,
    answer_note,
    back_to_todo,
    check_note,
    check_review_mode,
    effective_review,
    is_agent,
    kind_of,
    status_for,
    statuses_of,
    touch,
    unfinished_dependencies,
    vocabulary,
)


class _TaskStateModel:
    """The machine's model: its `state` is the task's kind, starting from the task's status."""

    def __init__(self, kind):
        self.state = kind


class TaskMachine:
    """The status commands as transitions between the six kinds, with their guards.

    A guard raises AtmError with the message the CLI prints; transitions passes it through unchanged.
    """

    states = ["todo", "active", "review", "blocked", "done", "skipped"]

    def __init__(self, task, data, agent, archived, stamp):
        self.task = task
        self.data = data
        self.agent = agent
        self.archived = archived
        self.stamp = stamp
        self.vocab = vocabulary(data)
        self.current_status = task.get("status")
        self.current_kind = kind_of(self.vocab, self.current_status) or "todo"
        self._model = _TaskStateModel(self.current_kind)

        # Every transition checks the command's arguments last, after its own guards, as change_status did
        args = [self.arguments_valid]
        transitions = [
            # From review only once the review asked for rework or approved it (SR-3.9)
            {
                "trigger": "start",
                "source": ["todo", "review"],
                "dest": "active",
                "conditions": [self.review_decided, self.dependencies_done, *args],
            },
            {"trigger": "review", "source": "active", "dest": "review", "conditions": args},
            {
                "trigger": "done",
                "source": ["active", "review"],
                "dest": "done",
                "conditions": [self.review_mode_allows_done, *args],
            },
            {"trigger": "fail", "source": "active", "dest": "todo", "conditions": args},
            {"trigger": "release", "source": "active", "dest": "todo", "conditions": args},
            {"trigger": "reject", "source": "review", "dest": "todo", "conditions": args},
            # The review's outcome; the task stays in review until an agent takes it back (SR-6.1)
            {"trigger": "approve", "source": "review", "dest": "review", "conditions": args},
            {"trigger": "request_changes", "source": "review", "dest": "review", "conditions": args},
            {"trigger": "block", "source": ["todo", "active"], "dest": "blocked", "conditions": args},
            {"trigger": "unblock", "source": "blocked", "dest": "todo", "conditions": args},
            # Undo a done (or skipped) by mistake, or work that turns out unfinished
            {"trigger": "reopen", "source": ["done", "skipped"], "dest": "todo", "conditions": args},
            # A task that turns out not to be needed; reopen undoes it
            {"trigger": "skip", "source": ["todo", "blocked"], "dest": "skipped", "conditions": args},
        ]

        self.machine = Machine(
            model=self._model,
            states=TaskMachine.states,
            transitions=transitions,
            initial=self.current_kind,
            send_event=True,
            auto_transitions=False,
        )

    # ------------------------------------------------------------------
    # Guards
    # ------------------------------------------------------------------

    def review_decided(self, event):
        """start from review: only once the review asked for changes or approved it."""
        if self.current_kind == "review" and self.task.get("review_state") not in REWORK_STATES:
            raise AtmError(
                f"can't start {self.task['id']}: it waits for its review (atm task review approve or request-changes {self.task['id']} decides it)"
            )
        return True

    def dependencies_done(self, event):
        """start: every dependency is done (one only in the archive is)."""
        blocking = unfinished_dependencies(self.data, self.task, self.archived)
        if blocking:
            raise AtmError(
                f"can't start {self.task['id']}: waiting on " + ", ".join(f"{d} ({s})" for d, s in blocking)
            )
        return True

    def review_mode_allows_done(self, event):
        """done: an agent finishing a `blocker` or `async` task asks for the review instead (SR-3.8)."""
        check_review_mode(self.task, self.data, self.agent)
        return True

    def arguments_valid(self, event):
        """Every command: notes, reasons, questions and answers fit in a note; block says what it waits for."""
        kwargs = event.kwargs
        note, reason = kwargs.get("note"), kwargs.get("reason")
        if note is not None:
            check_note(note)
        if reason is not None:
            check_note(reason)
        for question in kwargs.get("questions") or ():
            check_note(question)
        if event.event.name == "block":
            if kwargs.get("waiting_on") not in WAITING_ON:
                raise AtmError(f"--on must be {' or '.join(WAITING_ON)}", 2)
            if not kwargs.get("questions") and not reason:
                raise AtmError("say what it waits for: --question (repeatable) or --reason", 2)
        for answer in kwargs.get("answers") or ():
            check_note(answer)
        return True

    # ------------------------------------------------------------------
    # Public entry point
    # ------------------------------------------------------------------

    def transition(self, command, **kwargs):
        """Run `command` on the task. Returns its status before the command; raises AtmError if it isn't allowed."""
        trigger = command.replace("-", "_")
        if command in ("done", "skip"):
            kwargs["resolution"] = self._check_resolution(command, kwargs)
        if not self.machine.get_transitions(trigger, source=self.current_kind):
            allowed = sorted(
                {t.source for t in self.machine.get_transitions(trigger)}, key=TaskMachine.states.index
            )
            expected = [s for kind in allowed for s in statuses_of(self.vocab, kind)]
            raise AtmError(
                f"can't {command} {self.task['id']}: it is {self.current_status}, not {' or '.join(expected)}"
            )
        # The guards raise with their message; one that returns False would leave the task unchanged
        if not getattr(self._model, trigger)(**kwargs):
            raise AtmError(f"can't {command} {self.task['id']}")
        self._apply(command, self._model.state, kwargs)
        return self.current_status

    def _check_resolution(self, command, kwargs):
        """done and skip record a resolution (SR-2.8); skip may name the task it duplicates."""
        resolution = kwargs.get("resolution") or ("done" if command == "done" else "wont_do")
        allowed = RESOLUTIONS if command == "done" else SKIP_RESOLUTIONS
        if resolution not in allowed:
            raise AtmError(f"--resolution must be one of {', '.join(allowed)}", 2)
        duplicate_of = kwargs.get("duplicate_of")
        if duplicate_of is not None and resolution != "duplicate":
            raise AtmError("--duplicate-of goes with --resolution duplicate", 2)
        if duplicate_of == self.task["id"]:
            raise AtmError(f"{self.task['id']} can't be a duplicate of itself", 2)
        return resolution

    # ------------------------------------------------------------------
    # Effects, once the transition is allowed
    # ------------------------------------------------------------------

    def _apply(self, command, new_kind, kwargs):
        """Record the command on the task, in the order change_status did (it shapes the key order in tasks.json)."""
        task, vocab, agent, stamp = self.task, self.vocab, self.agent, self.stamp
        note, reason = kwargs.get("note"), kwargs.get("reason")
        old_kind = self.current_kind

        # A block's or a review's start goes when the task moves on: keep each interval, so waiting time can be measured
        if old_kind == "blocked" and task.get("blocked_at"):
            add_interval(task, "blocks", task["blocked_at"], stamp, on=task.get("waiting_on"))
        elif old_kind == "review" and new_kind != "review" and task.get("review_requested_at"):
            # The round ends when the reviewer decided, with its outcome (SR-3.9)
            outcome = task.get("review_state") if task.get("review_state") in REWORK_STATES else None
            add_interval(task, "reviews", task["review_requested_at"], task.get("reviewed_at") or stamp, outcome=outcome)
        if new_kind != "review":
            task.pop("review_state", None)
            task.pop("reviewed_at", None)

        if new_kind == "todo":
            # Reopening, rejecting or failing a task undoes its work; releasing or unblocking it doesn't
            back_to_todo(task, vocab, agent, stamp, keep_commit=command not in ("reopen", "reject", "fail"))
            if kwargs.get("answered"):
                add_note(task, "answer", kwargs["answered"], agent, stamp, checked=False)
            if command == "fail":
                add_note(task, "failure", reason or "verification failed", agent, stamp)
            elif command == "reject":
                add_note(task, "rejection", reason, agent, stamp)
            elif command == "reopen":
                add_note(task, "note", f"Reopened: {reason}", agent, stamp)
            elif note:
                add_note(task, "handoff", note, agent, stamp)
            return

        if command in ("approve", "request-changes") and kwargs.get("status") is None:
            task["status"] = self.current_status  # it stays in its review status (a project may have several)
        else:
            task["status"] = status_for(vocab, new_kind, kwargs.get("status"))
        touch(task, agent, stamp)

        if command == "block":
            # Who started it stays visible while it waits; unblock returns it to to-do and clears the attempt
            task["waiting_on"] = kwargs.get("waiting_on")
            task["questions"] = list(kwargs.get("questions") or ())
            task["blocked_at"] = stamp
            if reason:
                add_note(task, "note", reason, agent, stamp)
        elif command == "review":
            # The work ends here: the review isn't agent work (DM-4.1); done keeps this timestamp
            task["review_requested_at"] = stamp
            task["review_state"] = "waiting"
            if note:
                add_note(task, "review", note, agent, stamp)
        elif command in ("approve", "request-changes"):
            # The task stays in review until an agent takes it back (`atm next` offers it first, SR-3.9)
            task["review_state"] = "approved" if command == "approve" else "changes_requested"
            task["reviewed_at"] = stamp
            if command == "request-changes":
                add_note(task, "rejection", reason, agent, stamp)
            else:
                add_note(task, "note", f"Approved: {note}" if note else "Approved", agent, stamp)
        elif command == "skip":
            for field in ("waiting_on", "questions", "blocked_at"):
                task.pop(field, None)
            task["skipped_at"] = stamp
            if kwargs["resolution"] != "wont_do":
                task["resolution"] = kwargs["resolution"]  # none means the default, won't do
            if kwargs.get("duplicate_of"):
                task["duplicate_of"] = kwargs["duplicate_of"]
            add_note(task, "note", f"Skipped: {reason}", agent, stamp)
        elif command == "start":
            task["started_at"] = stamp
            task.pop("review_requested_at", None)  # taken back from review: a new round of work
            task.pop("completed_at", None)
            task.pop("completed_by", None)
            task.pop("agent", None)  # the old name of started_by
            if agent:
                task["started_by"] = agent
        elif command == "done":
            task["completed_at"] = stamp
            if kwargs["resolution"] != "done":
                task["resolution"] = kwargs["resolution"]  # none means the default, done
            if agent:
                task["completed_by"] = agent
            if kwargs.get("commit"):
                task["commit"] = kwargs["commit"]
            # A batch task an agent finishes waits for the end-of-sprint review (SR-4.4): its note is what to check
            batch = old_kind == "active" and is_agent(agent) and effective_review(vocab, task) == "batch"
            if batch:
                task["sprint_review"] = "pending"
            if note:
                add_note(task, "review" if batch else "outcome", note, agent, stamp)


def change_status(
    task,
    data,
    command,
    reason=None,
    agent=None,
    status=None,
    note=None,
    commit=None,
    waiting_on=None,
    questions=(),
    answers=(),
    resolution=None,
    duplicate_of=None,
    *,
    archived,
):
    """Apply one status command to `task` (in `data`); raises AtmError if the transition isn't allowed. `archived` is
    the project's lookup of archived tasks (`start` needs every dependency done; one only in the archive is). `status`
    picks the new status when its kind has several; `agent` is who runs it. `start` records them as started_by,
    `done` as completed_by; `note` and `reason` become notes, `commit` the task's commit. `block` records who it
    waits on and the open questions; `unblock` turns them, with the answers, into an answer note. `done` and `skip`
    record a `resolution` (SR-2.8), `skip` also `duplicate_of` (the caller checks that task exists)."""
    # Each question was checked when the task was blocked, each answer when it is given: together they may be longer
    # than one note, and must still be recordable, or the task could never be unblocked
    asked = list(task.get("questions") or [])
    answered = answer_note(asked, list(answers)) if command == "unblock" and (asked or answers) else None
    machine = TaskMachine(task, data, agent, archived, now_iso())
    return machine.transition(
        command,
        reason=reason,
        status=status,
        note=note,
        commit=commit,
        waiting_on=waiting_on,
        questions=questions,
        answers=answers,
        resolution=resolution,
        duplicate_of=duplicate_of,
        answered=answered,
    )
