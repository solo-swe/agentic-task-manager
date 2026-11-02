"""atm-lib: reading tasks (docs/layered-architecture.md §5.1)."""

from ..common.errors import NotFound
from ..common.fields import priority_of, started_by


def view_task(task, archived, tasks, archived_lookup, vocab):
    """A task with what is derived from the rest of the project: its effective priority, its dependencies' statuses,
    the tasks related to it either way (`related` is stored on one side, SR-2.8) and those that duplicate it.

    `tasks` are the working tasks, `archived_lookup(id)` an archived task or None, `vocab` the vocabularies."""
    by_id = {t.get("id"): t for t in tasks}
    dependencies = []
    for d in task.get("dependencies") or []:
        if d in by_id:
            dependencies.append({"id": d, "status": by_id[d].get("status"), "archived": False})
        elif archived_lookup(d):
            dependencies.append({"id": d, "status": archived_lookup(d).get("status"), "archived": True})
        else:
            dependencies.append({"id": d, "status": None, "archived": False})
    own = [r for r in task.get("related") or [] if isinstance(r, str)]
    others = [
        t.get("id")
        for t in tasks
        if t is not task and task.get("id") in (t.get("related") or []) and t.get("id") not in own
    ]
    return {
        "task": task,
        "archived": archived,
        "priority": priority_of(vocab, task),
        "started_by": started_by(task),
        "dependencies": dependencies,
        "related": own + others,
        "duplicated_by": [t.get("id") for t in tasks if t.get("duplicate_of") == task.get("id")],
    }


def show_tasks(project, ids):
    """`atm show`: the tasks in full, archived ones too, in the order asked for."""
    store = project.store
    tasks = store.list_tasks()
    vocab = store.get_vocabularies()
    views = []
    for task_id in ids:
        task, archived = store.get_task(task_id), False
        if task is None:
            task, archived = store.get_archived_task(task_id), True
        if task is None:
            raise NotFound(f"no task {task_id}")
        views.append(view_task(task, archived, tasks, store.get_archived_task, vocab))
    return views
