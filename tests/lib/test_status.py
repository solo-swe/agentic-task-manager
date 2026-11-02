"""atm-lib's status changes (lib/status.py) on the in-memory store: each one a single store change (§3.3)."""

import unittest

from support import task

from atm.common.errors import AtmFailure
from atm.db.memory import MemoryStore
from atm.lib.project import Project
from atm.lib.status import add_task_note, change_task_status


def store(*tasks):
    return MemoryStore(tasks=list(tasks))


class StatusChanges(unittest.TestCase):
    def test_start_claims_the_task_in_the_same_change(self):
        s = store(task("T-1"))
        result = change_task_status(Project(s, actor="claude:a"), "start", "T-1")
        self.assertEqual((result["previous"], result["task"]["status"]), ("todo", "in_progress"))
        self.assertEqual(s.claims["claims"]["T-1"]["agent"], "claude:a")
        self.assertEqual({(c["entity"], c["id"]) for c in s.changes}, {("task", "T-1"), ("claim", "T-1")})

    def test_two_agents_one_task_the_second_is_refused(self):
        s = store(task("T-1"))
        change_task_status(Project(s, actor="claude:a"), "start", "T-1")
        with self.assertRaises(AtmFailure) as refused:
            change_task_status(Project(s, actor="claude:b"), "done", "T-1")
        self.assertEqual(refused.exception.code, 4)  # claimed by another agent
        self.assertEqual(s.tasks[0]["status"], "in_progress")  # nothing changed

    def test_done_drops_the_claim_and_names_what_is_now_ready(self):
        s = store(task("T-1"), task("T-2", dependencies=["T-1"]))
        a = Project(s, actor="claude:a")
        change_task_status(a, "start", "T-1")
        result = change_task_status(a, "done", "T-1", note="Shipped")
        self.assertEqual(result["unblocked"], ["T-2"])
        self.assertNotIn("T-1", s.claims["claims"])
        self.assertIn("T-1", s.claims["completed"])
        self.assertEqual(s.tasks[0]["notes"][-1]["text"], "Shipped")

    def test_block_records_the_wait_for_every_worktree(self):
        s = store(task("T-1"))
        change_task_status(Project(s, actor="claude:a"), "block", "T-1", on="user", questions=["Which one?"])
        self.assertEqual(s.tasks[0]["questions"], ["Which one?"])
        self.assertEqual(s.claims["waiting"]["T-1"]["waiting_on"], "user")

    def test_a_refused_change_changes_nothing(self):
        s = store(task("T-1", status="done"))
        with self.assertRaises(AtmFailure):
            change_task_status(Project(s, actor="claude:a"), "start", "T-1")
        self.assertEqual(s.tasks[0]["status"], "done")
        self.assertEqual(s.claims["claims"], {})

    def test_a_note(self):
        s = store(task("T-1"))
        add_task_note(Project(s, actor="Ada"), "T-1", "Worth knowing")
        self.assertEqual([(n["by"], n["text"]) for n in s.tasks[0]["notes"]], [("Ada", "Worth knowing")])
        self.assertEqual([c["entity"] for c in s.changes], ["task"])


class StoreChange(unittest.TestCase):
    """The contract of `change` (docs/layered-architecture.md §3.3), on the in-memory store; the JSON-in-git store's
    is the same code path the atm-cli tests drive through every write command."""

    def test_the_changes_are_recorded_before_and_after(self):
        s = store(task("T-1"), task("T-2"))

        def rename(work):
            work.tasks()[1]["name"] = "Renamed"
            work.claims()["claims"]["T-2"] = {"task": "T-2"}

        s.change(rename)
        self.assertEqual(
            [(c["entity"], c["id"], (c["before"] or {}).get("name"), (c["after"] or {}).get("name")) for c in s.changes],
            [("task", "T-2", "Task T-2", "Renamed"), ("claim", "T-2", None, None)],
        )

    def test_a_deferred_error_is_raised_after_the_change_is_kept(self):
        s = store(task("T-1"))

        def expire(work):
            work.tasks()[0]["status"] = "todo"
            work.tasks()[0]["name"] = "Kept"
            work.defer(AtmFailure("your lease expired", 4))

        with self.assertRaisesRegex(AtmFailure, "lease expired"):
            s.change(expire)
        self.assertEqual(s.tasks[0]["name"], "Kept")


if __name__ == "__main__":
    unittest.main()
