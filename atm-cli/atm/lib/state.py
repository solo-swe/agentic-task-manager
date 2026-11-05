"""Task state machine using the transitions library.

Declares all task status transitions as data, with guards (conditions) and
side effects (before/after hooks). Replaces the hand-rolled change_status()
in common/model.py.

The machine state is always a *kind* (todo, active, review, blocked, done,
skipped), never a specific status name. The task's `status` field holds the
project-specific status name (e.g. `in_progress` for kind `active`). Hooks map
between kind and name via status_for().
"""

from __future__ import annotations

from transitions import Machine

from ..common.core import AtmError, now_iso
from ..common.errors import Invalid
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
    touch,
    unfinished_dependencies,
    vocabulary,
)


class _TaskStateModel:
    """Wrapper that lets transitions.Machine read/write task['status'] as a kind.

    The machine's .state is always a kind (todo, active, review, blocked, done,
    skipped). The task dict's "status" field holds the project's specific status
    name (e.g. "in_progress" for kind "active"). The kind is tracked in
    "_kind_state" and derived from "status" on first read.
    """

    def __init__(self, task, vocab):
        self._task = task
        self._vocab = vocab
        self._kind_state = None

    @property
    def state(self):
        if self._kind_state is not None:
            return self._kind_state
        return kind_of(self._vocab, self._task.get("status")) or "todo"

    @state.setter
    def state(self, value):
        self._kind_state = value


class TaskMachine:
    """Declares all task status transitions as data.

    States are the six kinds. Transitions are the 12 status commands.
    Guards are conditions; side effects are before/after hooks.
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
        self._model = _TaskStateModel(task, self.vocab)
        self._model.state = self.current_kind

        # Declared transitions: trigger -> (source, dest, conditions, before, after)
        transitions = [
            # start: todo or review -> active
            {
                "trigger": "start",
                "source": ["todo", "review"],
                "dest": "active",
                "conditions": [self.dependencies_done, self.can_start_from_review],
                "before": [self.create_claim],
            },
            # review: active -> review
            {
                "trigger": "review",
                "source": "active",
                "dest": "review",
                "before": [self.release_claim],
            },
            # done: active or review -> done
            {
                "trigger": "done",
                "source": ["active", "review"],
                "dest": "done",
                "conditions": [self.check_done_review_mode],
                "before": [self.record_completion],
            },
            # fail: active -> todo
            {
                "trigger": "fail",
                "source": "active",
                "dest": "todo",
                "before": [self.wipe_attempt],
                "after": [self.add_failure_note],
            },
            # release: active -> todo
            {
                "trigger": "release",
                "source": "active",
                "dest": "todo",
                "before": [self.wipe_attempt],
                "after": [self.add_handoff_note],
            },
            # reject: review -> todo
            {
                "trigger": "reject",
                "source": "review",
                "dest": "todo",
                "before": [self.wipe_attempt],
                "after": [self.add_rejection_note],
            },
            # approve: review -> review (stays)
            {
                "trigger": "approve",
                "source": "review",
                "dest": "review",
                "before": [self.mark_approved],
            },
            # request-changes: review -> review (stays)
            {
                "trigger": "request_changes",
                "source": "review",
                "dest": "review",
                "before": [self.mark_changes_requested],
            },
            # block: todo or active -> blocked
            {
                "trigger": "block",
                "source": ["todo", "active"],
                "dest": "blocked",
                "before": [self.release_claim],
            },
            # unblock: blocked -> todo
            {
                "trigger": "unblock",
                "source": "blocked",
                "dest": "todo",
            },
            # reopen: done or skipped -> todo
            {
                "trigger": "reopen",
                "source": ["done", "skipped"],
                "dest": "todo",
            },
            # skip: todo or blocked -> skipped
            {
                "trigger": "skip",
                "source": ["todo", "blocked"],
                "dest": "skipped",
            },
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
    # Conditions (guards)
    # ------------------------------------------------------------------

    def dependencies_done(self, event):
        """start: all dependencies must be done."""
        blocking = unfinished_dependencies(self.data, self.task, self.archived)
        if blocking:
            raise Invalid(
                f"can't start {self.task['id']}: waiting on "
                + ", ".join(f"{d} ({s})" for d, s in blocking)
            )
        return True

    def can_start_from_review(self, event):
        """start from review: review must be decided (changes_requested or approved)."""
        if self.current_kind == "review":
            if self.task.get("review_state") not in REWORK_STATES:
                raise Invalid(
                    f"can't start {self.task['id']}: it waits for its review "
                    f"(atm approve or request-changes decides it)"
                )
        return True

    def check_done_review_mode(self, event):
        """done: by review mode blocker/async, an agent must review first."""
        if self.current_kind == "active":
            check_review_mode(self.task, self.data, self.agent)
        return True

    # ------------------------------------------------------------------
    # Before hooks
    # ------------------------------------------------------------------

    def create_claim(self, event):
        """start: record the claim."""
        # Claim recording is done by the caller (change_task_status), not here.
        # This hook is a placeholder for future use.
        pass

    def release_claim(self, event):
        """review/block: release the claim."""
        # Claim release is done by the caller (change_task_status).
        pass

    def record_completion(self, event):
        """done: record completion timestamp and by."""
        if self.agent:
            self.task["completed_by"] = self.agent
        self.task["completed_at"] = self.stamp
        if event.kwargs.get("commit"):
            self.task["commit"] = event.kwargs["commit"]

    def wipe_attempt(self, event):
        """fail/release/reject: undo the work attempt."""
        keep_commit = event.kwargs.get("command") in ("fail", "release")
        back_to_todo(self.task, self.vocab, self.agent, self.stamp, keep_commit=keep_commit)

    def mark_approved(self, event):
        """approve: set review_state to approved."""
        self.task["review_state"] = "approved"
        self.task["reviewed_at"] = self.stamp
        note = event.kwargs.get("note")
        if note:
            add_note(self.task, "note", f"Approved: {note}" if note else "Approved", self.agent, self.stamp)

    def mark_changes_requested(self, event):
        """request-changes: set review_state to changes_requested."""
        self.task["review_state"] = "changes_requested"
        self.task["reviewed_at"] = self.stamp
        reason = event.kwargs.get("reason")
        if reason:
            add_note(self.task, "rejection", reason, self.agent, self.stamp)

    # ------------------------------------------------------------------
    # After hooks
    # ------------------------------------------------------------------

    def add_failure_note(self, event):
        """fail: add a failure note."""
        reason = event.kwargs.get("reason")
        add_note(self.task, "failure", reason or "verification failed", self.agent, self.stamp)

    def add_handoff_note(self, event):
        """release: add a handoff note."""
        note = event.kwargs.get("note")
        if note:
            add_note(self.task, "handoff", note, self.agent, self.stamp)

    def add_rejection_note(self, event):
        """reject: add a rejection note."""
        reason = event.kwargs.get("reason")
        add_note(self.task, "rejection", reason, self.agent, self.stamp)

    # ------------------------------------------------------------------
    # Public entry point
    # ------------------------------------------------------------------

    def transition(self, command, **kwargs):
        """Run the state machine transition for `command`.

        Returns the previous status (before the transition).
        Raises AtmError if the transition isn't allowed.
        """
        prev_status = self.current_status

        # Pre-transition validation that isn't expressible as a condition
        self._pre_validate(command, kwargs)

        # Map command to trigger name (request-changes -> request_changes)
        trigger = command.replace("-", "_")

        try:
            getattr(self._model, trigger)(**kwargs)
        except Exception as exc:
            # transitions wraps errors in MachineError; convert to AtmError
            raise AtmError(f"can't {command} {self.task['id']}: {exc}", 2) from exc

        # Post-transition field updates
        self._post_transition(command, kwargs)

        # The machine state is now a kind; map back to a specific status name
        new_kind = self._model.state
        chosen_status = kwargs.get("status")
        trigger = command.replace("-", "_")
        if trigger in ("approve", "request_changes") and chosen_status is None:
            # Stays in the current review status
            new_status = self.current_status
        else:
            new_status = status_for(self.vocab, new_kind, chosen_status)
        self.task["status"] = new_status
        touch(self.task, self.agent, self.stamp)

        return prev_status

    def _pre_validate(self, command, kwargs):
        """Validation that must happen before the state machine transition."""
        reason = kwargs.get("reason")
        note = kwargs.get("note")
        questions = kwargs.get("questions", [])
        answers = kwargs.get("answers", [])
        duplicate_of = kwargs.get("duplicate_of")
        resolution = kwargs.get("resolution")

        # done/skip: validate resolution
        if command in ("done", "skip"):
            resolution = resolution or ("done" if command == "done" else "wont_do")
            allowed_resolutions = RESOLUTIONS if command == "done" else SKIP_RESOLUTIONS
            if resolution not in allowed_resolutions:
                raise AtmError(f"--resolution must be one of {', '.join(allowed_resolutions)}", 2)
            if duplicate_of is not None and resolution != "duplicate":
                raise AtmError("--duplicate-of goes with --resolution duplicate", 2)
            if duplicate_of == self.task["id"]:
                raise AtmError(f"{self.task['id']} can't be a duplicate of itself", 2)

        # block: validate waiting_on and questions/reason
        if command == "block":
            waiting_on = kwargs.get("waiting_on")
            if waiting_on not in WAITING_ON:
                raise AtmError(f"--on must be {' or '.join(WAITING_ON)}", 2)
            if not questions and not reason:
                raise AtmError("say what it waits for: --question (repeatable) or --reason", 2)

        # Validate note/reason/question/answer lengths
        if note is not None:
            check_note(note)
        if reason is not None:
            check_note(reason)
        for question in questions:
            check_note(question)
        for answer in answers:
            check_note(answer)

    def _post_transition(self, command, kwargs):
        """Field updates after the state machine transition."""
        reason = kwargs.get("reason")
        note = kwargs.get("note")
        questions = kwargs.get("questions", [])
        answers = kwargs.get("answers", [])
        duplicate_of = kwargs.get("duplicate_of")
        resolution = kwargs.get("resolution")
        waiting_on = kwargs.get("waiting_on")
        stamp = self.stamp

        # done/skip: set default resolution
        if command in ("done", "skip"):
            resolution = resolution or ("done" if command == "done" else "wont_do")

        # Blocks and reviews intervals
        old_kind = self.current_kind
        new_kind = self._model.state

        if old_kind == "blocked" and self.task.get("blocked_at"):
            add_interval(self.task, "blocks", self.task["blocked_at"], stamp, on=self.task.get("waiting_on"))
        elif old_kind == "review" and new_kind != "review" and self.task.get("review_requested_at"):
            outcome = self.task.get("review_state") if self.task.get("review_state") in REWORK_STATES else None
            add_interval(
                self.task, "reviews",
                self.task["review_requested_at"],
                self.task.get("reviewed_at") or stamp,
                outcome=outcome,
            )

        # Transition out of review: clear review state
        if new_kind != "review":
            self.task.pop("review_state", None)
            self.task.pop("reviewed_at", None)

        # Kind-specific field updates
        if new_kind == "todo":
            self._back_to_todo(command, kwargs, stamp, answered=kwargs.get("answered"))
        elif command == "block":
            self.task["waiting_on"] = waiting_on
            self.task["questions"] = list(questions)
            self.task["blocked_at"] = stamp
            if reason:
                add_note(self.task, "note", reason, self.agent, stamp)
        elif command == "review":
            self.task["review_requested_at"] = stamp
            self.task["review_state"] = "waiting"
            if note:
                add_note(self.task, "review", note, self.agent, stamp)
        elif command == "skip":
            for field in ("waiting_on", "questions", "blocked_at"):
                self.task.pop(field, None)
            self.task["skipped_at"] = stamp
            if resolution != "wont_do":
                self.task["resolution"] = resolution
            if duplicate_of:
                self.task["duplicate_of"] = duplicate_of
            add_note(self.task, "note", f"Skipped: {reason}", self.agent, stamp)
        elif command == "start":
            self.task["started_at"] = stamp
            self.task.pop("review_requested_at", None)
            self.task.pop("completed_at", None)
            self.task.pop("completed_by", None)
            self.task.pop("agent", None)
            if self.agent:
                self.task["started_by"] = self.agent
        elif command == "done":
            if resolution != "done":
                self.task["resolution"] = resolution
            if self.agent:
                self.task["completed_by"] = self.agent
            if kwargs.get("commit"):
                self.task["commit"] = kwargs["commit"]
            batch = (
                old_kind == "active"
                and is_agent(self.agent)
                and effective_review(self.vocab, self.task) == "batch"
            )
            if batch:
                self.task["sprint_review"] = "pending"
            if note:
                add_note(
                    self.task, "review" if batch else "outcome", note, self.agent, stamp
                )

    def _back_to_todo(self, command, kwargs, stamp, answered=None):
        """Undo the work attempt when moving to todo."""
        keep_commit = command not in ("reopen", "reject", "fail")
        back_to_todo(self.task, self.vocab, self.agent, stamp, keep_commit=keep_commit)

        if answered:
            add_note(self.task, "answer", answered, self.agent, stamp, checked=False)

        if command == "fail":
            add_note(self.task, "failure", kwargs.get("reason") or "verification failed", self.agent, stamp)
        elif command == "reject":
            add_note(self.task, "rejection", kwargs.get("reason"), self.agent, stamp)
        elif command == "reopen":
            add_note(self.task, "note", f"Reopened: {kwargs.get('reason')}", self.agent, stamp)
        elif kwargs.get("note"):
            add_note(self.task, "handoff", kwargs.get("note"), self.agent, stamp)


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
    """Apply one status command to `task` using the transitions state machine.

    Same signature and behavior as common.model.change_status, but the state
    transition logic is declared as data in TaskMachine rather than hand-rolled
    if/elif chains.
    """
    stamp = now_iso()

    # Build the answered string for unblock before creating the machine
    asked = list(task.get("questions") or [])
    answered = (
        answer_note(asked, list(answers))
        if command == "unblock" and (asked or answers)
        else None
    )

    machine = TaskMachine(task, data, agent, archived, stamp)

    return machine.transition(
        command,
        reason=reason,
        agent=agent,
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
