"""atm-lib: all of atm's business logic (docs/layered-architecture.md §5), and its public surface for other tools.

Its functions take a Project and plain values and return results; they never parse arguments, print or exit, and
reach data only through the store's interface. Function names are unique across atm (`show_tasks`, not
`tasks.show`): the build puts every module in one namespace.

**For other tools** (an MCP server, a script), with `atm-cli/` on the import path; there is no published package:

    from atm.boot import open_project              # picks the stores: the JSON-in-git store and git
    from atm.lib import change_task_status, show_tasks

    project = open_project("/path/to/project", actor="my-tool")
    change_task_status(project, "start", "T-1")
    print(show_tasks(project, ["T-1"])[0]["task"]["status"])

What this module exports (`__all__`) is the public surface; everything else in atm, the other `atm.lib.*` names
included, is internal and may change in any release. The surface is versioned with the plugin
(`plugin/.claude-plugin/plugin.json`): a release that changes it says so in `plugin/CHANGELOG.md`. Failures are
`AtmFailure` subclasses (`NotFound`, `Invalid`, `Conflict`, `NothingAvailable`, `Claimed`), each with the exit code
the CLI gives it (`code`). A test can open a project on the in-memory store (`atm.db.memory.MemoryStore`) instead.
"""

from ..common.errors import AtmFailure, Claimed, Conflict, Invalid, NothingAvailable, NotFound
from .archive import archive_finished, archive_hint_now, unarchive_tasks
from .claims import live_claims, next_task_for, renew_lease, selection
from .code import commit_task, record_commit, record_merge, record_pr, set_workflow, workflow_settings
from .components import add_component, edit_component, list_components
from .edit import Changes, add_new_tasks, check_project, edit_existing_task
from .project import Project
from .projects import add_project, list_projects, prune_projects, remove_project
from .query import Filters, list_claims, list_matching, project_summary, search_tasks, status_report
from .query import task_dependencies, task_views
from .releases import add_release, archive_release, edit_release, list_releases, release_notes
from .settings import get_logo, get_repository, init_meta, set_logo, set_repository, unset_logo
from .sprints import add_sprint, backlog_tasks, close_sprint, delete_sprint, edit_sprint, list_sprints, move_tasks
from .sprints import start_sprint
from .status import add_task_note, change_task_status
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
    "list_matching",
    "task_views",
    "task_dependencies",
    "search_tasks",
    "status_report",
    "project_summary",
    # Tasks: status changes and notes (`change_task_status(project, "done", id, note=...)`)
    "change_task_status",
    "add_task_note",
    # Tasks: definitions
    "Changes",
    "add_new_tasks",
    "edit_existing_task",
    "check_project",
    # The loop: claims and the next task
    "selection",
    "next_task_for",
    "renew_lease",
    "live_claims",
    "list_claims",
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
    "workflow_settings",
    "set_workflow",
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
