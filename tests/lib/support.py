"""What atm-lib's tests share: atm-cli's modules on the import path, and small builders for task data.

The tests import the modules from atm-cli/atm/ as a package (the build turns the same modules into
plugin/scripts/atm.py). Run them with `node tests/scripts/run.mjs` (tests/scripts/lib.test.mjs runs this folder), or
`python3 -m unittest discover -s tests/lib -t tests/lib` from the repository's root.
"""

import os
import sys

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(REPO, "atm-cli"))


def task(task_id, **fields):
    """A task with the fields every task has, and any others given."""
    return {"id": task_id, "name": f"Task {task_id}", "type": "feature", "status": "todo", "dependencies": [], **fields}
