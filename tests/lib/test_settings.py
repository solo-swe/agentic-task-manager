"""atm-lib's settings (lib/settings.py), projects (lib/projects.py) and working copies (lib/worktrees.py), on the
in-memory store."""

import unittest

from support import task

from atm.common.errors import AtmFailure, Invalid
from atm.db.memory import MemoryStore
from atm.lib.project import Project
from atm.lib.projects import add_project, list_projects
from atm.lib.settings import (
    ensure_project_id,
    ensure_repository,
    get_logo,
    get_repository,
    init_meta,
    set_logo,
    set_repository,
    unset_logo,
)
from atm.lib.worktrees import worktree_copies


class Remote:
    def __init__(self, url):
        self.url = url

    def remote_url(self):
        return self.url


class Init(unittest.TestCase):
    def test_creates_the_task_data_then_merges_into_its_meta(self):
        s = MemoryStore(created=False)
        p = Project(s)
        self.assertEqual(init_meta(p, {"project": "Demo", "id": "ignored"}), (True, ["project"]))
        self.assertTrue(s.exists())
        self.assertEqual(s.meta["project"], "Demo")
        self.assertNotIn("id", s.meta)
        self.assertIn("created", s.meta)
        self.assertEqual(init_meta(p, {"description": "One line"}), (False, ["description"]))
        self.assertEqual((s.meta["project"], s.meta["description"]), ("Demo", "One line"))

    def test_an_unusable_vocabulary_writes_nothing(self):
        s = MemoryStore(created=False)
        with self.assertRaises(AtmFailure):
            init_meta(Project(s), {"statuses": [{"id": "x", "label": "X", "kind": "todo"}]})
        self.assertFalse(s.exists())

    def test_the_project_id_is_given_once(self):
        s = MemoryStore(tasks=[])
        first = ensure_project_id(Project(s))
        self.assertRegex(first, r"^[0-9a-f]{8}$")
        self.assertIsNone(ensure_project_id(Project(s)))
        self.assertIsNone(ensure_project_id(Project(MemoryStore(created=False))))


class Repository(unittest.TestCase):
    def test_from_the_remote_when_none_is_set(self):
        s = MemoryStore(tasks=[])
        p = Project(s, Remote("git@github.com:o/r.git"))
        self.assertEqual(ensure_repository(p), "https://github.com/o/r")
        self.assertIsNone(ensure_repository(Project(s, Remote("git@github.com:o/other.git"))))
        self.assertEqual(get_repository(p), "https://github.com/o/r")

    def test_a_move_takes_the_pr_links_along_in_the_archive_too(self):
        pr = lambda n: {"number": n, "url": f"https://github.com/o/r/pull/{n}"}
        s = MemoryStore(
            tasks=[task("T-1", pr=pr(1))], archive=[task("T-0", pr=pr(2))], meta={"repository": "https://github.com/o/r"}
        )
        result = set_repository(Project(s), "git@github.com:new/r.git")
        self.assertEqual((result["repository"], result["moved_links"]), ("https://github.com/new/r", 2))
        self.assertEqual(s.tasks[0]["pr"]["url"], "https://github.com/new/r/pull/1")
        self.assertEqual(s.archive[0]["pr"]["url"], "https://github.com/new/r/pull/2")
        with self.assertRaises(Invalid):
            set_repository(Project(s), "not a remote")


class Logo(unittest.TestCase):
    def test_a_store_without_files_has_no_logo_and_refuses_one(self):
        s = MemoryStore(tasks=[], meta={"logo": "logo.svg"})
        p = Project(s)
        self.assertEqual(get_logo(p), {"found": None, "set": "logo.svg"})
        with self.assertRaises(Invalid):
            set_logo(p, "other.svg")
        self.assertEqual(unset_logo(p), "logo.svg")
        self.assertIsNone(unset_logo(p))


class FakeRegistry:
    def __init__(self):
        self.entries = []

    def list_projects(self):
        return {"projects": [{**e, "missing": False} for e in self.entries], "damaged": None}

    def add_project(self, target):
        entry = {"id": "p1", "name": "One", "path": target}
        self.entries.append(entry)
        return {"result": "added", "project": entry, "damaged": None}


class Projects(unittest.TestCase):
    def test_through_the_registry_interface(self):
        registry = FakeRegistry()
        self.assertEqual(add_project(registry, "/x")["result"], "added")
        self.assertEqual([p["path"] for p in list_projects(registry)["projects"]], ["/x"])


class WorkingCopies(unittest.TestCase):
    def test_a_store_without_working_copies_has_none(self):
        self.assertEqual(worktree_copies(Project(MemoryStore(tasks=[task("T-1")]))), [])


if __name__ == "__main__":
    unittest.main()
