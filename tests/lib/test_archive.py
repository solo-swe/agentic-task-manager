"""atm-lib's archive (lib/archive.py) on the in-memory store."""

import unittest

from support import task

from atm.common.errors import AtmFailure
from atm.db.memory import MemoryStore
from atm.lib.archive import archive_finished, unarchive_tasks
from atm.lib.project import Project

OLD = "2020-01-01T00:00:00Z"


class Archive(unittest.TestCase):
    def test_moves_the_old_finished_tasks_and_keeps_the_rest_saying_why(self):
        s = MemoryStore(
            tasks=[
                task("T-1", status="done", completed_at=OLD),
                task("T-2", status="done", completed_at=OLD, pr={"number": 3}),
                task("T-3"),
            ]
        )
        result = archive_finished(Project(s))
        self.assertEqual(result["moved"], ["T-1"])
        self.assertEqual(result["kept"], [{"id": "T-2", "reason": "in open PR #3"}])
        self.assertEqual([t["id"] for t in s.tasks], ["T-2", "T-3"])
        self.assertEqual([t["id"] for t in s.archive], ["T-1"])
        self.assertEqual((s.meta["archived"], s.meta["next_id"]), (1, 4))

    def test_a_dry_run_changes_nothing(self):
        s = MemoryStore(tasks=[task("T-1", status="done", completed_at=OLD)])
        result = archive_finished(Project(s), dry_run=True)
        self.assertEqual((result["moved"], result["dry_run"]), (["T-1"], True))
        self.assertEqual(([t["id"] for t in s.tasks], s.archive), (["T-1"], []))

    def test_unarchive_brings_back_and_says_what_moves_back_next(self):
        s = MemoryStore(tasks=[task("T-2")], archive=[task("T-1", status="done", completed_at=OLD)])
        result = unarchive_tasks(Project(s), ["T-1"])
        self.assertEqual(result, {"restored": ["T-1"], "moves_back": ["T-1"]})
        self.assertEqual(([t["id"] for t in s.tasks], s.archive), (["T-2", "T-1"], []))

    def test_unarchive_names_the_two_mistakes(self):
        s = MemoryStore(tasks=[task("T-2")], archive=[])
        with self.assertRaisesRegex(AtmFailure, r"T-2 \(already in tasks.json\); T-9 \(no such task\)"):
            unarchive_tasks(Project(s), ["T-2", "T-9"])


if __name__ == "__main__":
    unittest.main()
