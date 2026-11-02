"""atm-lib's public surface (atm/lib/__init__.py), used as another tool would: only `atm.lib` for the work, and a store
to run it on (the in-memory one, or `atm.boot.open_project` on a project's folder)."""

import json
import os
import shutil
import tempfile
import unittest

import support  # noqa: F401 - atm-cli on the import path

import atm.lib as atm_lib
from atm.boot import open_project
from atm.db.memory import MemoryStore
from atm.lib import Project, add_new_tasks, change_task_status, next_task_for, selection, show_tasks


class PublicSurface(unittest.TestCase):
    def test_every_exported_name_is_there(self):
        missing = [name for name in atm_lib.__all__ if not hasattr(atm_lib, name)]
        self.assertEqual(missing, [])

    def test_a_task_from_start_to_done_on_the_in_memory_store(self):
        project = Project(MemoryStore(tasks=[]), actor="tool")
        added = add_new_tasks(project, [{"id": "new-1", "name": "Write it", "type": "feature"}])
        task_id = added["tasks"][0]["id"]
        claimed = next_task_for(project, selection(), claim=True)
        self.assertEqual((claimed["task"]["id"], claimed["claim"]["agent"]), (task_id, "tool"))
        change_task_status(project, "done", task_id, note="Written")
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
        change_task_status(open_project(root, actor="tool"), "start", "T-1")
        change_task_status(open_project(root, actor="tool"), "done", "T-1")
        with open(os.path.join(root, ".agentic-task-manager", "tasks.json"), encoding="utf-8") as f:
            self.assertEqual(json.load(f)["tasks"][0]["status"], "done")


if __name__ == "__main__":
    unittest.main()
