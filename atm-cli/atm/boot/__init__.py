"""boot: where a project is opened. The one place that picks the store and the version control for a project, so
atm-lib knows only their interfaces and the CLI and the REST API never touch storage (docs/layered-architecture.md
§1).

For other tools, `open_project` (and `open_registry`, the user's list of projects) are the public entry beside
`atm.lib` (see its docstring)."""

from .open import open_project, open_registry

__all__ = ["open_project", "open_registry"]
