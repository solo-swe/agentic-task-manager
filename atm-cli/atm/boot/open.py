"""Opening the project a command runs in."""

from ..db.jsongit.store import JsonGitStore
from ..lib.project import Project
from ..paths import find_paths


def open_project(cwd, actor=None):
    """The project at or above `cwd`, kept in its task files (the JSON-in-git store)."""
    return Project(JsonGitStore(find_paths(cwd)), actor=actor)
