"""Tolerant reads (T-377): a malformed task file is read as far as it can be, for showing it, and what is wrong is
said; atm's commands still refuse it (the strict read)."""

import json
import os
import shutil
import tempfile
import unittest

from support import task

from atm.common.errors import AtmFailure
from atm.db.jsongit.store import JsonGitStore
from atm.db.memory import MemoryStore
from atm.lib.project import Project
from atm.lib.query import Filters, read_problems, task_views
from atm.db.jsongit.paths import find_paths

MALFORMED = {
    "meta": {"project": "Broken", "priorities": "not a list", "default_priority": "P9"},
    "tasks": [
        task("T-1", status="done"),
        "not a task",
        {"name": "no id"},
        task("T 2", dependencies=["T-1"]),
        task("T-3", dependencies="T-1"),
        task("T-4", dependencies=["T-1"]),
    ],
}


class TolerantJsonGitStore(unittest.TestCase):
    def setUp(self):
        self.root = tempfile.mkdtemp(prefix="atm-tolerant-")
        self.addCleanup(shutil.rmtree, self.root)
        os.makedirs(os.path.join(self.root, ".agentic-task-manager"))
        with open(os.path.join(self.root, ".agentic-task-manager", "tasks.json"), "w", encoding="utf-8") as f:
            json.dump(MALFORMED, f)
        self.paths = find_paths(self.root)

    def test_the_strict_read_refuses_it(self):
        with self.assertRaises(AtmFailure):
            JsonGitStore(self.paths).list_tasks()

    def test_the_tolerant_read_keeps_what_it_can_and_says_what_is_wrong(self):
        store = JsonGitStore(self.paths, tolerant=True)
        self.assertEqual([t["id"] for t in store.list_tasks()], ["T-1", "T 2", "T-3", "T-4"])
        problems = store.problems()
        self.assertEqual(len([p for p in problems["file"] if "isn't a task" in p]), 2)
        self.assertTrue(any(p.startswith("meta.priorities") for p in problems["file"]))
        self.assertEqual(sorted(problems["tasks"]), ["T 2", "T-3"])
        # The vocabulary falls back to the defaults for what doesn't work
        self.assertEqual(store.get_vocabularies()["default_priority"], "P2")

    def test_the_views_mark_the_tasks_with_problems_not_ready(self):
        project = Project(JsonGitStore(self.paths, tolerant=True))
        derived = {v["task"]["id"]: v["derived"] for v in task_views(project, Filters())}
        self.assertTrue(derived["T-4"]["ready"])
        self.assertNotIn("problems", derived["T-4"])
        self.assertFalse(derived["T-3"]["ready"])
        self.assertEqual(
            derived["T-3"]["problems"], ["its dependencies aren't a list of task IDs (shown with the IDs that can be read)"]
        )
        self.assertFalse(derived["T 2"]["ready"])
        self.assertEqual(len(read_problems(project)), 3)  # two entries left out, one vocabulary

    def test_every_task_read_fits_the_schema(self):
        from schema_check import load_schema, problems

        schema = load_schema("task.schema.json")
        for t in JsonGitStore(self.paths, tolerant=True).list_tasks():
            self.assertEqual(problems(t, schema), [], t["id"])

    def test_writes_stay_strict(self):
        with self.assertRaises(AtmFailure):
            JsonGitStore(self.paths, tolerant=True).change(lambda work: None)


class ProblemsOnTheMemoryStore(unittest.TestCase):
    def test_a_clean_store_has_none(self):
        self.assertEqual(MemoryStore(tasks=[task("T-1")]).problems(), {"file": [], "tasks": {}})

    def test_given_problems_reach_the_views(self):
        store = MemoryStore(tasks=[task("T-1")], problems={"tasks": {"T-1": ["something"]}, "file": ["more"]})
        [view] = task_views(Project(store), Filters())
        self.assertEqual((view["derived"]["ready"], view["derived"]["problems"]), (False, ["something"]))
        self.assertEqual(read_problems(Project(store)), ["more"])


if __name__ == "__main__":
    unittest.main()
