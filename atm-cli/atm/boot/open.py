"""Opening a project: its task files (the JSON-in-git store) and git."""

from ..db.jsongit.store import JsonGitStore
from ..lib.project import Project
from ..paths import find_paths
from ..vcs.git import GitVcs


def open_project(cwd, actor=None):
    """The project at or above `cwd`."""
    return project_at(find_paths(cwd), actor)


def project_at(paths, actor=None):
    """The project whose task files `paths` locates (atm serve opens each registered project this way)."""
    return Project(JsonGitStore(paths), GitVcs(paths["root"]), actor)
