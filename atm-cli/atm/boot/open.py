"""Opening a project: its task files (the JSON-in-git store) and git."""

from ..db.jsongit.store import JsonGitStore
from ..lib.project import Project
from ..paths import find_paths
from ..vcs.git import GitVcs


def open_project(cwd, actor=None, tolerant=False):
    """The project at or above `cwd`. `tolerant`: read a malformed task file as far as it can be read (T-377)."""
    return project_at(find_paths(cwd), actor, tolerant)


def project_at(paths, actor=None, tolerant=False):
    """The project whose task files `paths` locates (atm serve opens each registered project this way)."""
    return Project(JsonGitStore(paths, tolerant=tolerant), GitVcs(paths["root"]), actor)
