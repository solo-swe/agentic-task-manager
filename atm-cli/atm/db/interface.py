"""The store: how atm-lib reads and writes a project's data, whatever keeps it (docs/layered-architecture.md §3).

It grows with the migration, one slice at a time (§9): each slice adds the operations it needs. Tasks are the
dicts of the task file's format (docs/v4/data-model.md §4); a store may hand out its own objects, so atm-lib never
changes what a read returns.
"""

from typing import List, Optional, Protocol


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
