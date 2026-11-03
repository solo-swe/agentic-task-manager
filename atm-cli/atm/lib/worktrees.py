"""atm-lib: a project's working copies (git worktrees, for the JSON-in-git store; docs/layered-architecture.md §3.2):
the dashboard shows them on one board, each task from the working copy where it got furthest."""

from ..common.errors import AtmFailure
from .project import Project
from .query import Filters, task_views


def working_copies(project):
    """The project's working copies, main first ([{path, branch, main}]); [] for a store without them."""
    return project.store.working_copies()


def copy_views(project, folder):
    """The tasks of the working copy at `folder` as views ({task, derived}), each worked out on that copy's own data
    (§11.10 A): what `atm next` would say there, so a dependency done in another worktree counts once the branches
    merge. By task ID; {} when the copy can't be read."""
    try:
        views = task_views(Project(project.store.copy_store(folder)), Filters())
    except AtmFailure:
        return {}
    return {v["task"].get("id"): v for v in views if isinstance(v["task"].get("id"), str)}


def worktree_copies(project):
    """The project's working copies, main first ([{path, branch, main}]), and for each other one the tasks whose copy
    there differs from the main one's (or that only it has), under `tasks`, as views ({task, derived}) worked out in
    that working copy. A copy of a task decided in another working copy (answered, reviewed: the claims store's
    `resolved`, T-204) is left out: it is stale until atm next runs there, and would otherwise outrank the decision.
    [] for a store without working copies."""
    store = project.store
    trees = store.working_copies()
    resolved = store.read_claims()["resolved"]
    stale = {(r.get("worktree"), task_id) for task_id, r in resolved.items() if isinstance(r, dict)}
    listed, main = [], {}
    for tree in trees:
        tasks = store.copy_tasks(tree["path"])
        if tree["main"]:
            main = {t["id"]: t for t in tasks}
            listed.append(tree)
            continue
        differing = [t for t in tasks if main.get(t["id"]) != t and (tree["path"], t["id"]) not in stale]
        views = copy_views(project, tree["path"]) if differing else {}
        listed.append(
            {**tree, "tasks": [views.get(t["id"]) or {"task": t, "derived": {}} for t in differing]}
        )
    return listed
