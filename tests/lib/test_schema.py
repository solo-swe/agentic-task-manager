"""common's schemas describe the data atm writes and the dashboard reads (docs/layered-architecture.md §4): every
task in the dashboard's fixtures fits task.schema.json, and so does every field of the tasks in the task files the
scenario tests expect (written by atm; minimal on purpose, so without the required fields)."""

import glob
import json
import os
import unittest

from schema_check import load_schema, problems
from support import REPO

TASK = load_schema("task.schema.json")
META = load_schema("meta.schema.json")


def tasks_in(document):
    """The tasks of a task file; some scenarios keep malformed entries on purpose, which aren't tasks."""
    tasks = document.get("tasks", []) if isinstance(document, dict) else []
    return [t for t in tasks if isinstance(t, dict)] if isinstance(tasks, list) else []


class TaskSchema(unittest.TestCase):
    def assert_fits(self, tasks, source, required=True):
        for t in tasks:
            self.assertEqual(problems(t, TASK, required=required), [], f"{source}: {t.get('id')}")

    def test_the_dashboard_fixtures(self):
        files = glob.glob(os.path.join(REPO, "dashboard", "tests", "fixtures", "*.json"))
        self.assertTrue(files)
        for path in files:
            with open(path, encoding="utf-8") as f:
                document = json.load(f)
            self.assert_fits(tasks_in(document), os.path.basename(path))
            if isinstance(document, dict) and isinstance(document.get("meta"), dict):
                self.assertEqual(problems(document["meta"], META), [], f"{os.path.basename(path)}: meta")

    def test_the_meta_the_scenarios_expect(self):
        checked = 0
        for path in glob.glob(os.path.join(REPO, "tests", "scripts", "scenarios", "*.json")):
            with open(path, encoding="utf-8") as f:
                scenarios = json.load(f)
            for scenario in scenarios if isinstance(scenarios, list) else []:
                if (scenario.get("expect") or {}).get("exit", 0) != 0:
                    continue  # a file atm refuses, kept as it was on purpose
                for name, content in ((scenario.get("expect") or {}).get("files") or {}).items():
                    if name.endswith("tasks.json") and isinstance(content, dict) and isinstance(content.get("meta"), dict):
                        source = f"{os.path.basename(path)}: {scenario.get('name')}"
                        self.assertEqual(problems(content["meta"], META, required=False), [], source)
                        checked += 1
        self.assertGreater(checked, 0)

    def test_the_default_vocabularies_fit_meta(self):
        with open(os.path.join(REPO, "atm-cli", "atm", "common", "schema", "vocabularies.json"), encoding="utf-8") as f:
            defaults = json.load(f)
        meta = {k: defaults[k] for k in ("statuses", "types", "priorities", "default_priority")}
        self.assertEqual(problems(meta, META), [])
        self.assertEqual([s["kind"] for s in defaults["statuses"]], defaults["kinds"])

    def test_the_task_files_the_scenarios_expect(self):
        checked = 0
        for path in glob.glob(os.path.join(REPO, "tests", "scripts", "scenarios", "*.json")):
            with open(path, encoding="utf-8") as f:
                scenarios = json.load(f)
            for scenario in scenarios if isinstance(scenarios, list) else scenarios.get("scenarios", []):
                for name, content in ((scenario.get("expect") or {}).get("files") or {}).items():
                    if name.endswith(("tasks.json", "tasks-archive.json")) and isinstance(content, dict):
                        source = f"{os.path.basename(path)}: {scenario.get('name')}"
                        self.assert_fits(tasks_in(content), source, required=False)
                        checked += 1
        self.assertGreater(checked, 0)

    def test_the_generated_python_types_load(self):
        # Every type refers to the others by name, wherever in the file they are defined
        from atm.common import types

        self.assertIn("derived", types.TaskView.__annotations__)
        self.assertIn("notes", types.Task.__annotations__)

    def test_a_task_that_does_not_fit_is_named(self):
        self.assertEqual(problems({"id": "T-1", "name": "x", "type": "feature"}, TASK), ["$: missing 'status'"])
        self.assertEqual(
            problems({"id": "T-1", "name": "x", "type": "f", "status": "todo", "pr": {"number": "7"}}, TASK),
            ["$.pr.number: expected integer, got str"],
        )


if __name__ == "__main__":
    unittest.main()
