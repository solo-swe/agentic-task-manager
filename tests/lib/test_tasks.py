"""atm-lib's task reads (lib/tasks.py), on the in-memory store."""

import unittest

from support import task

from atm.common.errors import NotFound
from atm.db.memory import MemoryStore
from atm.lib.project import Project
from atm.lib.tasks import show_tasks


def project(tasks, archive=()):
    return Project(MemoryStore(tasks=tasks, archive=list(archive)))


class ShowTasks(unittest.TestCase):
    def test_in_the_order_asked_for(self):
        views = show_tasks(project([task("T-1"), task("T-2")]), ["T-2", "T-1"])
        self.assertEqual([v["task"]["id"] for v in views], ["T-2", "T-1"])
        self.assertEqual([v["archived"] for v in views], [False, False])

    def test_an_archived_task_says_so(self):
        [view] = show_tasks(project([task("T-2")], [task("T-1", status="done")]), ["T-1"])
        self.assertTrue(view["archived"])

    def test_an_unknown_task_is_an_error(self):
        with self.assertRaisesRegex(NotFound, "no task T-9"):
            show_tasks(project([task("T-1")]), ["T-1", "T-9"])

    def test_dependencies_with_their_status_archived_or_missing(self):
        tasks = [task("T-2", status="in_progress"), task("T-3", dependencies=["T-2", "T-1", "T-0"])]
        [view] = show_tasks(project(tasks, [task("T-1", status="done")]), ["T-3"])
        self.assertEqual(
            view["dependencies"],
            [
                {"id": "T-2", "status": "in_progress", "archived": False},
                {"id": "T-1", "status": "done", "archived": True},
                {"id": "T-0", "status": None, "archived": False},
            ],
        )

    def test_related_either_way_and_duplicates(self):
        tasks = [task("T-1", related=["T-2"]), task("T-2"), task("T-3", related=["T-2"]), task("T-4", duplicate_of="T-2")]
        [view] = show_tasks(project(tasks), ["T-2"])
        self.assertEqual(view["related"], ["T-1", "T-3"])
        self.assertEqual(view["duplicated_by"], ["T-4"])

    def test_the_effective_priority_and_who_started_it(self):
        old = task("T-1", priority="P9", agent="claude:old")
        [view] = show_tasks(project([old]), ["T-1"])
        self.assertEqual(view["priority"], "P2")  # unknown: the default
        self.assertEqual(view["started_by"], "claude:old")  # `agent` from before v4


if __name__ == "__main__":
    unittest.main()
