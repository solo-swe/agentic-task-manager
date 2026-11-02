"""atm-lib: a project's working copies (git worktrees, for the JSON-in-git store; docs/layered-architecture.md §3.2):
the dashboard shows them on one board, each task from the working copy where it got furthest."""


def worktree_copies(project):
    """The project's working copies, main first ([{path, branch, main}]), and for each other one the tasks whose copy
    there differs from the main one's (or that only it has), under `tasks`. A copy of a task decided in another
    working copy (answered, reviewed: the claims store's `resolved`, T-204) is left out: it is stale until atm next
    runs there, and would otherwise outrank the decision. [] for a store without working copies."""
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
        else:
            differing = [t for t in tasks if main.get(t["id"]) != t and (tree["path"], t["id"]) not in stale]
            listed.append({**tree, "tasks": differing})
    return listed
