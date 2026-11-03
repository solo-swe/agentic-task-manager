"""atm-lib's public surface (atm/lib/__init__.py and atm/boot/__init__.py), used as another tool would: only `atm.lib`
for the work and `atm.boot` to open a project (in memory, or on a project's folder); and frozen."""

import inspect
import json
import os
import shutil
import tempfile
import unittest

import support  # noqa: F401 - atm-cli on the import path

import atm.boot as atm_boot
import atm.lib as atm_lib
from atm.boot import open_memory_project, open_project
from atm.lib import add_new_tasks, done_task, next_task_for, selection, show_tasks, start_task
from schema_check import load_schema, problems

API = load_schema("api.schema.json")
FROZEN = os.path.join(os.path.dirname(os.path.abspath(__file__)), "public-surface.json")


def surface():
    """The public surface as it is: each module's exported names, with each one's signature (a class: `class` and
    its constructor's)."""

    def described(value):
        try:
            signature = str(inspect.signature(value))
        except (TypeError, ValueError):
            signature = ""
        return f"class{signature}" if inspect.isclass(value) else signature

    return {
        module.__name__: {name: described(getattr(module, name)) for name in sorted(module.__all__)}
        for module in (atm_lib, atm_boot)
    }


class FrozenSurface(unittest.TestCase):
    """§11.6 (T-384): the names and signatures other tools rely on change only on purpose. A change to the surface
    fails here until `tests/lib/public-surface.json` is updated with it (the message has the new list), and goes in
    `plugin/CHANGELOG.md`."""

    def test_the_surface_is_the_frozen_one(self):
        with open(FROZEN, encoding="utf-8") as f:
            frozen = json.load(f)
        current = surface()
        self.assertEqual(
            current,
            frozen,
            "atm's public surface changed: if on purpose, put this in tests/lib/public-surface.json and say so in "
            "plugin/CHANGELOG.md\n" + json.dumps(current, indent=2) + "\n",
        )

    def test_tools_need_nothing_from_atm_db(self):
        # The surface's own example opens a project in memory without importing a store
        project = open_memory_project(tasks=[{"id": "T-1", "name": "One", "type": "feature", "status": "todo"}])
        self.assertEqual(start_task(project, "T-1")["task"]["status"], "in_progress")
        self.assertNotIn("atm.db", " ".join(atm_boot.__all__ + atm_lib.__all__))


class PublicSurface(unittest.TestCase):
    def test_every_exported_name_is_there(self):
        missing = [name for name in atm_lib.__all__ if not hasattr(atm_lib, name)]
        self.assertEqual(missing, [])

    def test_one_function_per_status_change_and_no_command_names(self):
        # §5.1, T-383: change_task_status(project, "done", ...) is internal; so are the CLI-shaped option objects
        self.assertFalse({"change_task_status", "Changes", "list_matching", "live_claims"} & set(atm_lib.__all__))

    def test_a_status_change_answers_as_the_schema_says(self):
        project = open_memory_project(actor="tool")
        task_id = add_new_tasks(project, [{"name": "Write it", "type": "feature"}])["tasks"][0]["id"]
        for result in (start_task(project, task_id), done_task(project, task_id, note="Written")):
            self.assertEqual(problems(result, API["$defs"]["StatusChange"], API), [])

    def test_a_task_from_start_to_done_on_the_in_memory_store(self):
        project = open_memory_project(actor="tool")
        added = add_new_tasks(project, [{"id": "new-1", "name": "Write it", "type": "feature"}])
        task_id = added["tasks"][0]["id"]
        claimed = next_task_for(project, selection(), claim=True)
        self.assertEqual((claimed["task"]["id"], claimed["claim"]["agent"]), (task_id, "tool"))
        done_task(project, task_id, note="Written")
        task = show_tasks(project, [task_id])[0]["task"]
        self.assertEqual((task["status"], task["updated_by"]), ("done", "tool"))
        nothing = next_task_for(project, selection(), claim=True)
        self.assertEqual((nothing["task"], nothing["reason"]), (None, "nothing left to do"))

    def test_a_project_folder_through_boot(self):
        root = tempfile.mkdtemp(prefix="atm-public-")
        self.addCleanup(shutil.rmtree, root)
        os.makedirs(os.path.join(root, ".agentic-task-manager"))
        with open(os.path.join(root, ".agentic-task-manager", "tasks.json"), "w", encoding="utf-8") as f:
            json.dump({"meta": {"project": "Tool"}, "tasks": [{"id": "T-1", "name": "One", "type": "feature", "status": "todo"}]}, f)
        start_task(open_project(root, actor="tool"), "T-1")
        done_task(open_project(root, actor="tool"), "T-1")
        with open(os.path.join(root, ".agentic-task-manager", "tasks.json"), encoding="utf-8") as f:
            self.assertEqual(json.load(f)["tasks"][0]["status"], "done")


if __name__ == "__main__":
    unittest.main()
