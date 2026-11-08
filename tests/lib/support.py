"""What atm-lib's tests share: atm-cli's modules on the import path, and small builders for task data.

The tests import the modules from atm-cli/atm/ as a package (the build turns the same modules into
plugin/scripts/atm.py), and the third-party code atm-cli uses from atm-cli/vendored/, the same copy the build puts
into the bundle, so they run with a plain Python and nothing installed. Run them with `node tests/scripts/run.mjs`
(tests/scripts/lib.test.mjs runs this folder), or `python3 -m unittest discover -s tests/lib -t tests/lib` from the
repository's root.
"""

import os
import sys

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
VENDORED = os.path.join(REPO, "atm-cli", "vendored")
# A vendored package is a folder (vendored/transitions/); a single-module one is its module in a folder of its name
# (vendored/six/six.py), so that folder goes on the path too: a module found there wins over the namespace folder
sys.path[:0] = [os.path.join(REPO, "atm-cli"), VENDORED, os.path.join(VENDORED, "six")]

# The generated modules (atm.common.types, atm.cli.agent_docs, …) are in .generated/atm-cli/atm/, which
# atm-cli/build.py writes (tests/scripts/run.mjs runs it first): each package looks in its folder there too
GENERATED = os.path.join(REPO, ".generated", "atm-cli", "atm")
if not os.path.isdir(GENERATED):
    raise RuntimeError("no .generated/: run `python3 atm-cli/build.py --generated` (tests/scripts/run.mjs does)")
import atm.cli  # noqa: E402
import atm.common  # noqa: E402

for package in (atm.common, atm.cli):
    package.__path__.append(os.path.join(GENERATED, package.__name__.split(".")[1]))


def task(task_id, **fields):
    """A task with the fields every task has, and any others given."""
    return {"id": task_id, "name": f"Task {task_id}", "type": "feature", "status": "todo", "dependencies": [], **fields}
