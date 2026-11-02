"""Where a project's task files are: the project root, the task folder, legacy locations."""

import os

from .common.core import DASHBOARD_FILE, TASK_DIR, TASKS_FILE, AtmError, out, to_json
from .db.jsongit.gitrun import git

LEGACY_DIRS = ["docs", "."]
# The files atm reads and writes in the task folder; a repository can commit any of them as a symlink
FOLDER_FILES = (TASKS_FILE, "tasks-archive.json", ".gitignore", ".gitattributes")


def check_task_folder(root, task_dir):
    """Refuse a task folder atm would read or write outside the project through (docs/security-review.md F7): a
    cloned repository can commit `.agentic-task-manager` or its files as symlinks to anything the user can write,
    such as ~/.zshrc."""
    if os.path.islink(task_dir):
        raise AtmError(f"{task_dir} is a symlink; atm works only on a real folder inside the project")
    if not os.path.isdir(task_dir):
        return
    real_root = os.path.realpath(root)
    if os.path.commonpath([os.path.realpath(task_dir), real_root]) != real_root:
        raise AtmError(f"{task_dir} leads outside the project ({os.path.realpath(task_dir)})")
    for name in FOLDER_FILES:
        if os.path.islink(os.path.join(task_dir, name)):
            raise AtmError(f"{os.path.join(task_dir, name)} is a symlink; atm reads and writes only regular files there")


def project_root(cwd):
    """The git top level; outside git, the nearest directory with a task folder or docs/tasks.json, else cwd."""
    top = git(["rev-parse", "--show-toplevel"], cwd)
    if top:
        return top
    d = cwd
    while True:
        if os.path.isdir(os.path.join(d, TASK_DIR)) or os.path.isfile(os.path.join(d, "docs", TASKS_FILE)):
            return d
        parent = os.path.dirname(d)
        if parent == d:
            return cwd
        d = parent


def find_paths(cwd):
    root = project_root(cwd)
    task_dir = os.path.join(root, TASK_DIR)
    check_task_folder(root, task_dir)
    legacy = None
    for d in LEGACY_DIRS:
        candidate = os.path.normpath(os.path.join(root, d, TASKS_FILE))
        if os.path.isfile(candidate):
            dashboard = os.path.join(os.path.dirname(candidate), DASHBOARD_FILE)
            legacy = {"tasks": candidate, "dashboard": dashboard if os.path.isfile(dashboard) else None}
            break
    tasks = os.path.join(task_dir, TASKS_FILE)
    return {
        "root": root,
        "dir": task_dir,
        "tasks": tasks,
        "tasks_exists": os.path.isfile(tasks),
        "dashboard": os.path.join(task_dir, DASHBOARD_FILE),
        "legacy": legacy,
    }


def cmd_paths(args):
    paths = find_paths(os.getcwd())
    if args.json:
        out(to_json(paths))
        return
    for key in ["root", "dir", "tasks", "dashboard"]:
        out(f"{key}: {paths[key]}")
    if paths["legacy"]:
        out(f"legacy: {paths['legacy']['tasks']}")
