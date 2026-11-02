"""atm-lib's sprints, releases and backlog (lib/sprints.py, lib/releases.py) on the in-memory store."""

import unittest

from support import task

from atm.common.errors import AtmFailure
from atm.db.memory import MemoryStore
from atm.lib.edit import Changes
from atm.lib.project import Project
from atm.lib.releases import add_release, list_releases, release_notes
from atm.lib.sprints import add_sprint, backlog_tasks, close_sprint, list_sprints, move_tasks, start_sprint

# The options are named as the command line names them; what isn't given is None (Changes gives that)
options = Changes


class Sprints(unittest.TestCase):
    def test_add_start_close_carrying_the_unfinished(self):
        s = MemoryStore(tasks=[task("T-1"), task("T-2")])
        p = Project(s, actor="Ada")
        add_sprint(p, options(name="One", goal="First"))
        add_sprint(p, options(name="Two"))
        move_tasks(p, ["T-1", "T-2"], "One")
        start_sprint(p, "One")
        s.tasks[0]["status"] = "done"
        result = close_sprint(p, "One", options(carry_to="Two"))
        report = result["sprint"]["report"]
        self.assertEqual((report["done"], report["carried"], report["to"]), (["T-1"], ["T-2"], "Two"))
        self.assertEqual(s.tasks[1]["sprint"], "Two")
        self.assertTrue(result["was_active"])
        self.assertEqual([x["id"] for x in list_sprints(p, include_closed=True)["sprints"]], ["One", "Two"])

    def test_one_active_sprint_at_a_time_and_nothing_written_on_refusal(self):
        s = MemoryStore(tasks=[], meta={"sprints": [{"id": "A", "state": "active"}, {"id": "B", "state": "planned"}]})
        with self.assertRaisesRegex(AtmFailure, "'A' is active"):
            start_sprint(Project(s), "B")
        self.assertEqual(s.meta["sprints"][1]["state"], "planned")

    def test_the_backlog(self):
        s = MemoryStore(tasks=[task("T-1"), task("T-2", sprint="X"), task("T-3", status="done")], meta={"sprints": [{"id": "X"}]})
        self.assertEqual([t["id"] for t in backlog_tasks(Project(s))["tasks"]], ["T-1"])
        self.assertEqual([t["id"] for t in backlog_tasks(Project(s), finished=True)["tasks"]], ["T-1", "T-3"])


class Releases(unittest.TestCase):
    def test_add_from_a_pr_takes_its_tasks_archived_ones_too(self):
        pr = {"number": 7, "url": "https://example.com/pr/7"}
        s = MemoryStore(tasks=[task("T-2", status="done", pr=pr)], archive=[task("T-1", status="done", pr=pr)])
        result = add_release(Project(s), "1.0", options(pr=[7], released_at="2026-01-01T00:00:00Z"))
        self.assertEqual(sorted(result["tasks"]), ["T-1", "T-2"])
        self.assertEqual(s.archive[0]["release"], "1.0")
        self.assertEqual(result["release"]["state"], "released")
        [listed] = list_releases(Project(s))["releases"]
        self.assertEqual((listed["tasks"], listed["by_stage"]), (2, {"in_pr": 2}))

    def test_the_notes_draft(self):
        done = task("T-1", status="done", release="1.0", notes=[{"kind": "outcome", "text": "Shipped"}])
        s = MemoryStore(tasks=[done, task("T-2", release="1.0")], meta={"releases": [{"id": "1.0"}]})
        notes = release_notes(Project(s), "1.0")
        self.assertEqual(notes["groups"][0]["tasks"], [{"id": "T-1", "name": "Task T-1", "outcome": "Shipped"}])
        self.assertEqual([t["id"] for t in notes["open"]], ["T-2"])


if __name__ == "__main__":
    unittest.main()
