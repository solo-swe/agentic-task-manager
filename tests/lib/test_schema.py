"""common's schemas describe the data atm writes and the dashboard reads (docs/layered-architecture.md §4): every
task in the dashboard's fixtures fits task.schema.json, and so does every field of the tasks in the task files the
scenario tests expect (written by atm; minimal on purpose, so without the required fields); a project's summary and
status report fit api.schema.json."""

import glob
import json
import os
import unittest

from schema_check import load_schema, problems
from support import REPO, task

from atm.db.memory import MemoryStore
from atm.lib.project import Project
from atm.lib.query import Filters, project_summary, status_report

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


API = load_schema("api.schema.json")


def answer_problems(value, name):
    return problems(value, API["$defs"][name], API)


def busy_project():
    """A project with something in every part of the summary and the status report: an active sprint and a closed
    one with a task left open, a live claim, a review, a block on the user, work together with the user, a PR, an
    archived dependency."""
    later = "2999-01-01T00:00:00Z"
    sprints = [
        {"id": "S1", "state": "closed", "end": "2026-01-01T00:00:00Z"},
        {"id": "S2", "state": "active", "goal": "Ship", "due": "2026-12-01", "start": "2026-11-01T00:00:00Z"},
    ]
    tasks = [
        task("T-1", sprint="S1"),
        task("T-2", sprint="S2", status="in_progress", started_at="2026-11-01T10:00:00Z", started_by="claude:a"),
        task("T-3", sprint="S2", status="in_review", review_requested_at="2026-11-01T11:00:00Z"),
        task("T-4", sprint="S2", status="blocked", waiting_on="user", questions=["Which?"], blocked_at="2026-11-01T12:00:00Z"),
        task("T-5", sprint="S2", status="done", completed_at="2026-11-01T13:00:00Z", sprint_review="pending",
             pr={"number": 7, "url": "https://example.test/pr/7"}),
        task("T-6", sprint="S2", **{"with": "user"}, dependencies=["T-0"], labels=["ux"]),
        task("T-7", sprint="S2", dependencies=["T-2"], priority="P1"),
        task("T-8"),
    ]
    claims = {
        "claims": {"T-2": {"task": "T-2", "agent": "claude:a", "claimed_at": "2026-11-01T10:00:00Z",
                           "lease_expires_at": later, "branch": "dev"}},
        "completed": {}, "hints": {}, "resolved": {}, "waiting": {},
    }
    meta = {"project": "Busy", "description": "Everything at once", "sprints": sprints}
    return Project(MemoryStore(tasks=tasks, archive=[task("T-0", status="done")], meta=meta, claims=claims))


class AnswerSchema(unittest.TestCase):
    """api.schema.json's ProjectSummary and StatusReport describe what atm-lib answers (§11.10 B, T-382): the
    dashboard's types are generated from them, so a field atm sends differently breaks this test, not the page."""

    def assert_answers_fit(self, project, source):
        self.assertEqual(answer_problems(project_summary(project), "ProjectSummary"), [], f"{source}: summary")
        report = status_report(project, Filters())["report"]
        self.assertEqual(answer_problems(report, "StatusReport"), [], f"{source}: status")
        return report

    def test_a_busy_project(self):
        project = busy_project()
        summary = project_summary(project)
        # Every nullable part is filled in here, so its inner type is the one checked
        self.assertTrue(summary["active_sprint"] and summary["last_done"] and summary["git"]["pr"])
        self.assertTrue(summary["working"] and summary["needs_you_tasks"] and summary["board_active_sprint"])
        report = self.assert_answers_fit(project, "busy")
        self.assertTrue(report["sprint"] and report["blocked"] and report["in_review"] and report["in_progress"])
        self.assertTrue(report["open_in_closed_sprints"] and report["waiting_on_dependencies"])

    def test_an_empty_project(self):
        self.assert_answers_fit(Project(MemoryStore()), "empty")

    def test_the_dashboard_fixtures(self):
        # What the e2e suite's api/projects gives for a fixture (withAtmSummary: atm summary of what it serves)
        files = glob.glob(os.path.join(REPO, "dashboard", "tests", "fixtures", "*.json"))
        for path in files:
            with open(path, encoding="utf-8") as f:
                document = json.load(f)
            if not isinstance(document, dict) or not isinstance(document.get("tasks"), list):
                continue
            store = MemoryStore(tasks=tasks_in(document), meta=document.get("meta") or {})
            self.assert_answers_fit(Project(store), os.path.basename(path))

    def test_a_null_where_the_schema_does_not_allow_one_is_named(self):
        self.assertEqual(
            answer_problems({"count": None, "cards": []}, "BoardColumn"), ["$.count: expected integer, got NoneType"]
        )
        self.assertEqual(answer_problems({"branch": None, "pr": None}, "GitSummary"), [])
        self.assertEqual(
            answer_problems({"branch": None, "pr": {"number": 1, "url": None, "state": "closed"}}, "GitSummary"),
            ["$.pr.state: 'closed' is not one of ['open', 'merged']"],
        )


if __name__ == "__main__":
    unittest.main()
