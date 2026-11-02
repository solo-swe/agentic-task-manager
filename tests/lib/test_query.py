"""atm-lib's read commands (lib/query.py) on the in-memory store: list, deps, status and the summary."""

import unittest

from support import task

from atm.common.errors import AtmFailure, NotFound
from atm.db.memory import MemoryStore
from atm.lib.project import Project
from atm.lib.query import Filters, list_matching, project_summary, status_report, task_dependencies

SPRINTS = {"sprints": [{"id": "One", "state": "active", "goal": "First"}, {"id": "Two", "state": "planned"}]}


def project(tasks, archive=(), meta=None, claims=None):
    return Project(MemoryStore(tasks=tasks, archive=list(archive), meta=meta or {}, claims=claims))


def ids(tasks):
    return [t["id"] for t in tasks]


class ListMatching(unittest.TestCase):
    def setUp(self):
        self.project = project(
            [
                task("T-1", status="done", sprint="One"),
                task("T-2", dependencies=["T-1"], labels=["ux"], sprint="One"),
                task("T-3", dependencies=["T-2"], priority="P0", sprint="Two"),
                task("T-4", status="blocked", **{"with": "user"}),
            ],
            meta=SPRINTS,
        )

    def test_everything_without_filters(self):
        self.assertEqual(ids(list_matching(self.project, Filters())["tasks"]), ["T-1", "T-2", "T-3", "T-4"])

    def test_by_scope_and_fields(self):
        match = lambda **f: ids(list_matching(self.project, Filters(**f))["tasks"])
        self.assertEqual(match(sprint=["One"]), ["T-1", "T-2"])
        self.assertEqual(match(backlog=True), ["T-4"])
        self.assertEqual(match(label=["ux"]), ["T-2"])
        self.assertEqual(match(priority="P0"), ["T-3"])
        self.assertEqual(match(priority="P2"), ["T-1", "T-2", "T-4"])  # a missing priority is the default
        self.assertEqual(match(with_user=True), ["T-4"])

    def test_ready_and_blocked(self):
        self.assertEqual(ids(list_matching(self.project, Filters(ready=True))["tasks"]), ["T-2"])
        self.assertEqual(ids(list_matching(self.project, Filters(blocked=True))["tasks"]), ["T-3", "T-4"])

    def test_an_unknown_sprint_names_the_sprints(self):
        with self.assertRaisesRegex(NotFound, "no sprint 'Three'; the sprints: One, Two") as caught:
            list_matching(self.project, Filters(sprint=["Three"]))
        self.assertEqual(caught.exception.code, 2)  # as a usage error, as before

    def test_the_archive(self):
        found = list_matching(project([task("T-2")], [task("T-1", status="done")]), Filters(archived=True))
        self.assertEqual(ids(found["tasks"]), ["T-1"])


class TaskDependencies(unittest.TestCase):
    def test_both_ways_archived_ones_marked(self):
        p = project(
            [task("T-2", status="in_progress", dependencies=["T-1", "T-0"]), task("T-3", dependencies=["T-2"])],
            [task("T-1", status="done"), task("T-4", status="done", dependencies=["T-2"])],
        )
        deps = task_dependencies(p, "T-2")
        self.assertEqual([(d["id"], d["status"]) for d in deps["waits_for"]], [("T-1", "done, archived"), ("T-0", "missing")])
        self.assertEqual([(d["id"], d["status"]) for d in deps["waited_on_by"]], [("T-3", "todo"), ("T-4", "done, archived")])

    def test_an_unknown_task(self):
        with self.assertRaisesRegex(NotFound, "no task T-9"):
            task_dependencies(project([task("T-1")]), "T-9")


class StatusReport(unittest.TestCase):
    def test_the_active_sprint_counts_and_next(self):
        p = project(
            [
                task("T-1", status="done", sprint="One"),
                task("T-2", sprint="One"),
                task("T-3", status="in_progress", sprint="One", started_by="claude:a"),
            ],
            meta=SPRINTS,
        )
        status = status_report(p, Filters())
        report = status["report"]
        self.assertTrue(status["has_sprints"])
        self.assertEqual(report["sprint"]["id"], "One")
        self.assertEqual((report["sprint"]["done"], report["sprint"]["tasks"]), (1, 3))
        self.assertEqual(report["by_status"], {"todo": 1, "in_progress": 1, "done": 1})
        self.assertEqual(report["in_progress"], [{"id": "T-3", "name": "Task T-3", "by": "claude:a"}])
        self.assertEqual(report["next"], {"id": "T-2", "name": "Task T-2"})
        self.assertGreater(report["file_bytes"], 0)

    def test_without_sprints(self):
        status = status_report(project([task("T-1")]), Filters())
        self.assertFalse(status["has_sprints"])
        self.assertIsNone(status["report"]["sprint"])


class ProjectSummary(unittest.TestCase):
    def test_the_numbers_the_cards_show(self):
        claims = {"claims": {"T-2": {"task": "T-2", "agent": "claude:a", "lease_expires_at": "2999-01-01T00:00:00Z"}}}
        p = project(
            [task("T-1", status="done"), task("T-2", status="in_progress"), task("T-3"), task("T-4", status="skipped")],
            claims=claims,
        )
        summary = project_summary(p)
        self.assertEqual((summary["total"], summary["done"], summary["skipped"], summary["percent"]), (4, 1, 1, 33))
        self.assertEqual(summary["agents_working"], 1)
        self.assertEqual(summary["next"], [{"id": "T-3", "name": "Task T-3"}])
        self.assertIsNone(summary["git"]["branch"])  # no version control in this project

    def test_errors_are_atm_failures(self):
        self.assertTrue(issubclass(NotFound, AtmFailure))


if __name__ == "__main__":
    unittest.main()
