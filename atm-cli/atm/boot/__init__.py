"""boot: where a project is opened. The one place that picks the store and the version control for a project, so
atm-lib knows only their interfaces and the CLI and the REST API never touch storage (docs/layered-architecture.md
§1).

For other tools, this module's `__all__` is the public entry beside `atm.lib` (see its docstring): `open_project`
(a project's folder, on the JSON-in-git store and git), `open_memory_project` (a project in memory, for tests) and
`open_registry` (the user's list of projects). The two modules are the whole public surface; their names and
signatures are frozen by `tests/lib/test_public.py`."""

from .open import open_memory_project, open_project, open_registry

__all__ = ["open_project", "open_memory_project", "open_registry"]
