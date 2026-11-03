"""Opening a project, and the user's registry of projects: what every store has (docs/layered-architecture.md §1,
§11.2). Boot is the one place that knows which store and which version control atm runs on: today the JSON-in-git
store and git. What only the JSON-in-git store has (its paths, migrations, merge driver, registering by folder) is
`boot/jsongit.py`'s."""

from ..db.jsongit.paths import check_task_folder, find_paths
from ..db.jsongit.store import JsonGitStore
from ..db.memory import MemoryStore
from ..db.registry import FileRegistry
from ..lib.project import Project
from ..vcs.git import GitVcs


def open_project(cwd, actor=None, tolerant=False):
    """The project at or above `cwd`. `tolerant`: read a malformed task file as far as it can be read (T-377)."""
    return _project_at(find_paths(cwd), actor, tolerant)


def project_opener(folder):
    """A way to open the project at `folder` again and again, cheaply: `opener(actor=None, tolerant=False)` gives the
    project afresh each time (atm serve, one snapshot per request), finding where its data is only once (that runs
    git). Its folder's symlink checks run on every open (docs/security-review.md F7, F8)."""
    found = {}

    def opener(actor=None, tolerant=False):
        if "paths" not in found:
            found["paths"] = find_paths(folder)
        return _project_at(found["paths"], actor, tolerant)

    return opener


def _project_at(paths, actor=None, tolerant=False):
    """The project whose task files `paths` locates. The task folder's symlink checks run each time, so a file linked
    after the paths were found is refused too."""
    check_task_folder(paths["root"], paths["dir"])
    return Project(JsonGitStore(paths, tolerant=tolerant), GitVcs(paths["root"]), actor)


def open_memory_project(tasks=(), archive=(), meta=None, actor=None):
    """A project on the in-memory store, with no version control: for other tools' tests (and atm-lib's own), so they
    never import `atm.db`. The lists given are copied; the project's changes stay in memory."""
    copy = lambda items: [dict(item) for item in items]
    return Project(MemoryStore(tasks=copy(tasks), archive=copy(archive), meta=dict(meta or {})), None, actor)


def open_registry(home=None):
    """The user's registry of projects (the dashboard's list)."""
    return FileRegistry(home)
