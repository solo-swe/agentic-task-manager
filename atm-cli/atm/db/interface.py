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

    def get_settings(self) -> dict:
        """The project's settings: meta without its lists (project, description, id, repository, logo, workflow,
        review, archive, …)."""
        ...

    def list_sprints(self) -> List[dict]:
        """The sprints, in order, checked (each with its state)."""
        ...

    def list_releases(self) -> List[dict]:
        """The releases, in order, checked."""
        ...

    def list_components(self) -> List[dict]:
        """The components, with their globs, checked."""
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

    def files_for_commit(self) -> List[str]:
        """The files of the store that travel with a code commit (relative to the repository): tasks.json for the
        JSON-in-git store, none for a store that syncs elsewhere (docs/layered-architecture.md §2)."""
        ...

    def problems(self) -> dict:
        """What a tolerant read found wrong with the data (T-377): {"file": [...], "tasks": {id: [...]}}, in `atm
        check`'s words; empty for a strict read, which refuses such data instead."""
        ...

    def working_copy(self) -> Optional[str]:
        """The working copy this store reads, for a store that has several (git worktrees); None otherwise."""
        ...

    def raw(self) -> dict:
        """The data as stored, unchecked, for `atm check` to report on: {"document" (None when it isn't task data),
        "archive" (the archived tasks), "archive_error" (why the archive can't be read, or None)}."""
        ...

    def change(self, fn: Callable[[Any], Any]) -> Any:
        """Run `fn(work)` (a db.working.Working: the tasks and claims to change) under the store's isolation, and write
        what it changed; nothing when it raises. Returns what `fn` returns, then raises what it deferred. The
        transitional form of `apply` (docs/layered-architecture.md §3.3): `work.changes()` is the list of writes."""
        ...

    def document(self) -> dict:
        """The whole task document (`meta` and `tasks`), for the rules that still take it whole."""
        ...

    def exists(self) -> bool:
        """Whether the project has task data yet (`atm init` creates it)."""
        ...

    def create(self, document: dict) -> None:
        """Store the project's first task data; Conflict if it has some already."""
        ...

    def stamp(self) -> dict:
        """What changes when the data does, for the REST API's ETags: per part ({tasks, archive, claims, head, copies}),
        a value that changes whenever that part does; None for a part that isn't there."""
        ...

    def logo(self, meta: dict) -> Optional[dict]:
        """The project's logo for the dashboard: {path, file, type, source: "set" or "found"}, or None."""
        ...

    def check_logo(self, relative: str) -> None:
        """Refuse (Invalid) a logo the dashboard can't be given."""
        ...

    def working_copies(self) -> List[dict]:
        """The project's working copies, main first ([{path, branch, main}]); [] for a store without them."""
        ...

    def copy_tasks(self, folder: str) -> List[dict]:
        """The tasks as the working copy at `folder` has them."""
        ...


class Registry(Protocol):
    """The user's list of projects, the ones the dashboard shows (docs/layered-architecture.md §3.2). Each operation
    also says where an unreadable list was moved (`damaged`), or None."""

    def list_projects(self) -> dict:
        """{projects: the entries ({id, name, path, added_at, seen_at, missing}), damaged}."""
        ...

    def add_project(self, target: Optional[str]) -> dict:
        """Register the project in the folder `target`. {result, project, damaged}."""
        ...

    def remove_project(self, target: str) -> dict:
        """Unregister the project with this ID or path. {removed, damaged}."""
        ...

    def prune(self) -> dict:
        """Unregister the projects that are gone. {removed, damaged}."""
        ...

    def entries(self) -> List[dict]:
        """The entries as they are now, read without waiting (atm serve, on every request)."""
        ...

    def stamp(self) -> Any:
        """What changes when the list does."""
        ...
