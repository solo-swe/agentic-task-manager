"""The commands about the JSON-in-git store itself (docs/layered-architecture.md §3.2, §11.2): `atm paths`,
`migrate`, `merge-driver` and the lock test. They exist for this store (its files, its git merge driver, its lock), and
reach it only through `boot/jsongit.py`: a store that isn't files in git has none of them."""

import os
import sys

from ..boot.jsongit import hold_lock_for_test, merge_versions, migrate_folder, task_paths
from ..common.core import AtmExit, out, to_json


def cmd_migrate(args):
    migration = migrate_folder(os.getcwd())
    if args.json:
        out(to_json(migration["result"]))
        return
    for line in migration["lines"]:
        out(line)


def cmd_merge_driver(args):
    conflicts = merge_versions(args.base, args.ours, args.theirs)
    if args.json:
        out(to_json({"conflicts": conflicts}))
    if conflicts:
        sys.stderr.write("atm: tasks.json has conflicting edits (this branch's version was kept):\n")
        for conflict in conflicts:
            sys.stderr.write(f"  {conflict}\n")
        raise AtmExit(1)


def cmd_paths(args):
    show_paths(task_paths(os.getcwd()), args.json)


def show_paths(paths, as_json):
    """Print where the task files are (`atm paths`, `atm2 project config`)."""
    if as_json:
        out(to_json(paths))
        return
    for key in ["root", "dir", "tasks", "dashboard"]:
        out(f"{key}: {paths[key]}")
    if paths["legacy"]:
        out(f"legacy: {paths['legacy']['tasks']}")


def cmd_lock_test(args):
    """Test hook: hold the task folder's lock for `seconds`, so tests can check exclusion."""
    hold_lock_for_test(os.getcwd(), args.seconds)
