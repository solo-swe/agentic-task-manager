"""The store: how atm-lib reads and writes a project's data, whatever keeps it (docs/layered-architecture.md §3).

It grows with the migration, one slice at a time (§9): each slice adds the operations it needs. Tasks are the
dicts of the task file's format (docs/v4/data-model.md §4); a store may hand out its own objects, so atm-lib never
changes what a read returns.
"""

from typing import Any, Callable, List, Optional, Protocol


class Store(Protocol):
    def get_vocabularies(self) -> dict:
        """The project's statuses, types, priorities and the rest of its vocabularies, with the defaults filled in."""
        ...

    def list_tasks(self) -> List[dict]:
        """The working tasks (not the archived ones), in the project's order."""
        ...

    def get_task(self, task_id: str) -> Optional[dict]:
        """A working task, or None."""
        ...

    def get_archived_task(self, task_id: str) -> Optional[dict]:
        """An archived task, or None."""
        ...

    def list_archived(self) -> List[dict]:
        """The archived tasks."""
        ...

    def read_claims(self) -> dict:
        """The claims and what goes with them: `claims` (by task), `completed`, `waiting` and `resolved` (tasks done,
        waiting or decided in another working copy) and `hints` (when each hint was last shown)."""
        ...

    def sizes(self) -> dict:
        """How big the project's data is, in bytes: `tasks` (the working tasks) and `archive`."""
        ...

    def problems(self) -> dict:
        """What a tolerant read found wrong with the data (T-377): {"file": [...], "tasks": {id: [...]}}, in `atm
        check`'s words; empty for a strict read, which refuses such data instead."""
        ...

    def working_copy(self) -> Optional[str]:
        """The working copy this store reads, for a store that has several (git worktrees); None otherwise."""
        ...

    def change(self, fn: Callable[[Any], Any]) -> Any:
        """Run `fn(work)` (a db.working.Working: the tasks and claims to change) under the store's isolation, and write
        what it changed; nothing when it raises. Returns what `fn` returns, then raises what it deferred. The
        transitional form of `apply` (docs/layered-architecture.md §3.3): `work.changes()` is the list of writes."""
        ...

    def document(self) -> dict:
        """The whole task document (`meta` and `tasks`), for the code from before the layers that still takes it.
        Transitional: it goes when the last of that code has moved (docs/layered-architecture.md §9, step 14)."""
        ...
