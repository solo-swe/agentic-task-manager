"""atm-lib: all of atm's business logic (docs/layered-architecture.md §5), and its public surface for other tools.

Its functions take a Project and plain values (keyword parameters where there are several) and return results; they
never parse arguments, print or exit, and reach data only through the store's interface. Function names are unique
across atm (`show_tasks`, not `tasks.show`): the build puts every module in one namespace. §5.1 lists them.

**For other tools** (an MCP server, a script), importing from the repository's `atm-cli/` (on the import path), not
from the plugin's shipped `plugin/scripts/atm.py`, which is one flat file built for the CLI; there is no published
package:

    from atm.boot import open_project              # picks the stores: the JSON-in-git store and git
    from atm.lib import done_task, show_tasks, start_task

    project = open_project("/path/to/project", actor="my-tool")
    start_task(project, "T-1")
    done_task(project, "T-1", note="Shipped")
    print(show_tasks(project, ["T-1"])[0]["task"]["status"])

The public surface is two modules: what this one exports (`__all__`) and `atm.boot`'s `__all__` (`open_project`,
`open_memory_project`, `open_registry`). Everything else in atm, the other `atm.lib.*` and `atm.db` names included,
is internal and may change in any release. The names and every function's signature are frozen in
`tests/lib/public-surface.json`, which `tests/lib/test_public.py` compares with the code: a change fails it until the
list is updated, and goes in `CHANGELOG.md` (the surface is versioned with the plugin,
`plugin/.claude-plugin/plugin.json`). Failures are `AtmFailure` subclasses (`NotFound`, `Invalid`, `Conflict`,
`NothingAvailable`, `Claimed`), each with the exit code the CLI gives it (`code`). A test opens a project in memory
with `atm.boot.open_memory_project(tasks=[...])`.

The results that cross to a front end are typed in `crates/atm-common/schema/api.schema.json` (generated into
`atm.common.types`): `TaskView` (task_views), `StatusChange` (the status changes), `DepsResult`, `StatusReport`,
`ProjectSummary`.
"""

from ..common.errors import AtmFailure, Claimed, Conflict, Invalid, NothingAvailable, NotFound
from .archive import archive_finished, archive_hint_now, unarchive_tasks
from .claims import expire_claims, next_task_for, renew_due_claims, renew_lease, selection
from .code import (
    commit_task,
    folder_changes,
    get_workflow,
    record_commit,
    record_merge,
    record_pr,
    repository_paths,
    set_workflow,
)
from .components import add_component, edit_component, list_components
from .edit import add_new_tasks, check_project, edit_existing_task
from .project import Project
from .projects import add_project, list_projects, prune_projects, remove_project
from .query import Filters, list_claims, project_summary, search_tasks, status_report, task_dependencies, task_views
from .releases import add_release, archive_release, edit_release, list_releases, release_notes
from .settings import get_logo, get_repository, init_meta, set_logo, set_repository, unset_logo
from .sprints import add_sprint, backlog_tasks, close_sprint, delete_sprint, edit_sprint, list_sprints, move_tasks
from .sprints import start_sprint
from .status import add_task_note, approve_task, block_task, done_task, drop_task, fail_task, reject_task, release_task
from .status import reopen_task, request_changes, review_task, skip_task, start_task, unblock_task
from .tasks import show_tasks
from .worktrees import worktree_copies

__all__ = [
    # The project, and the errors
    "Project",
    "AtmFailure",
    "NotFound",
    "Invalid",
    "Conflict",
    "NothingAvailable",
    "Claimed",
    # Tasks: reading
    "show_tasks",
    "Filters",
    "task_views",
    "task_dependencies",
    "search_tasks",
    "status_report",
    "project_summary",
    # Tasks: status changes and notes
    "start_task",
    "review_task",
    "done_task",
    "reject_task",
    "drop_task",
    "approve_task",
    "request_changes",
    "block_task",
    "unblock_task",
    "reopen_task",
    "skip_task",
    "fail_task",
    "release_task",
    "add_task_note",
    # Tasks: definitions
    "add_new_tasks",
    "edit_existing_task",
    "check_project",
    # The loop: claims and the next task (list_claims reads; expire_claims and renew_* write)
    "selection",
    "next_task_for",
    "list_claims",
    "expire_claims",
    "renew_lease",
    "renew_due_claims",
    # Sprints, releases, components
    "list_sprints",
    "backlog_tasks",
    "add_sprint",
    "edit_sprint",
    "start_sprint",
    "close_sprint",
    "delete_sprint",
    "move_tasks",
    "list_releases",
    "add_release",
    "edit_release",
    "archive_release",
    "release_notes",
    "list_components",
    "add_component",
    "edit_component",
    # The archive
    "archive_finished",
    "unarchive_tasks",
    "archive_hint_now",
    # Code, PRs and the workflow
    "get_workflow",
    "set_workflow",
    "repository_paths",
    "folder_changes",
    "commit_task",
    "record_commit",
    "record_pr",
    "record_merge",
    # Settings
    "init_meta",
    "get_repository",
    "set_repository",
    "get_logo",
    "set_logo",
    "unset_logo",
    # The user's projects, and a project's working copies
    "list_projects",
    "add_project",
    "remove_project",
    "prune_projects",
    "worktree_copies",
]
