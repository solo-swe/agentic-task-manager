"""atm-lib's definitions (lib/edit.py, lib/components.py) on the in-memory store: add, edit, check, components."""

import unittest

from support import task

from atm.common.errors import AtmFailure
from atm.db.memory import MemoryStore
from atm.lib.components import add_component, edit_component, list_components
from atm.lib.edit import add_new_tasks, check_project, edit_existing_task
from atm.lib.project import Project


class AddAndEdit(unittest.TestCase):
    def test_add_gives_the_next_ids_and_resolves_placeholders(self):
        s = MemoryStore(tasks=[task("T-001")])
        result = add_new_tasks(
            Project(s, actor="Ada"),
            [{"id": "new-1", "name": "One", "type": "feature"}, {"name": "Two", "type": "test", "dependencies": ["new-1"]}],
        )
        self.assertEqual([t["id"] for t in result["tasks"]], ["T-002", "T-003"])
        self.assertEqual(s.tasks[2]["dependencies"], ["T-002"])
        self.assertEqual(s.tasks[1]["created_by"], "Ada")

    def test_add_is_all_or_nothing(self):
        s = MemoryStore(tasks=[task("T-001")])
        with self.assertRaises(AtmFailure):
            add_new_tasks(Project(s), [{"name": "One", "type": "feature"}, {"name": "Bad", "type": "nope"}])
        self.assertEqual([t["id"] for t in s.tasks], ["T-001"])

    def test_a_dependency_on_an_archived_task_is_fine(self):
        s = MemoryStore(tasks=[task("T-002")], archive=[task("T-001", status="done")])
        edit_existing_task(Project(s), "T-002", add_dependencies=["T-001"])
        self.assertEqual(s.tasks[0]["dependencies"], ["T-001"])

    def test_edit_says_what_changed_and_refuses_a_cycle(self):
        s = MemoryStore(tasks=[task("T-1"), task("T-2", dependencies=["T-1"])])
        result = edit_existing_task(Project(s, actor="Ada"), "T-1", name="Renamed", add_labels=["ux"])
        self.assertEqual(result["changed"], ["name", "labels"])
        self.assertEqual(s.tasks[0]["updated_by"], "Ada")
        with self.assertRaisesRegex(AtmFailure, "cycle"):
            edit_existing_task(Project(s), "T-1", add_dependencies=["T-2"])

    def test_edit_takes_keyword_parameters_and_an_empty_value_removes(self):
        s = MemoryStore(tasks=[task("T-1", review="blocker", files=["a.py"])])
        result = edit_existing_task(Project(s), "T-1", task_type="bugfix", review="", remove_files=["a.py"], with_="user")
        self.assertEqual(sorted(result["changed"]), ["files", "review", "type", "with"])
        self.assertEqual((s.tasks[0]["type"], s.tasks[0]["with"], s.tasks[0]["files"]), ("bugfix", "user", []))
        self.assertNotIn("review", s.tasks[0])

    def test_nothing_to_change_changes_nothing(self):
        s = MemoryStore(tasks=[task("T-1", name="Same")])
        result = edit_existing_task(Project(s, actor="Ada"), "T-1", name="Same")
        self.assertEqual(result["changed"], [])
        self.assertNotIn("updated_by", s.tasks[0])

    def test_requirements_and_use_cases_take_ids_and_links_but_not_empty_text(self):
        link = "[SR-2.1](docs/x.md#:~:text=SR%2D2.1)"
        s = MemoryStore(tasks=[task("T-1", requirements=["FR-1"])])
        result = edit_existing_task(
            Project(s), "T-1", add_requirements=[link], remove_requirements=["FR-1"], add_use_cases=["UC-1"]
        )
        self.assertEqual(result["changed"], ["requirements", "use_cases"])
        self.assertEqual((s.tasks[0]["requirements"], s.tasks[0]["use_cases"]), ([link], ["UC-1"]))
        with self.assertRaisesRegex(AtmFailure, "empty use case"):
            edit_existing_task(Project(s), "T-1", add_use_cases=[""], remove_requirements=[link])
        self.assertEqual(s.tasks[0]["requirements"], [link])


class Check(unittest.TestCase):
    def test_errors_and_warnings(self):
        s = MemoryStore(tasks=[task("T-1", dependencies=["T-9"]), task("T-1"), task("T-2", type="unknown")])
        result = check_project(Project(s))
        self.assertTrue(any("used by 2 tasks" in e for e in result["errors"]))
        self.assertTrue(any("depends on T-9" in e for e in result["errors"]))
        self.assertTrue(any("type 'unknown'" in w for w in result["warnings"]))
        self.assertEqual(result["count"], 3)


class Components(unittest.TestCase):
    def test_add_from_a_label_moves_it_on_archived_tasks_too(self):
        s = MemoryStore(tasks=[task("T-2", labels=["ui"])], archive=[task("T-1", status="done", labels=["ui"])])
        result = add_component(Project(s), "dashboard", paths=["dash/**"], from_label="ui")
        self.assertEqual(sorted(result["from_label"]), ["T-1", "T-2"])
        self.assertEqual(s.archive[0]["components"], ["dashboard"])
        self.assertNotIn("labels", s.archive[0])
        self.assertEqual([(c["id"], c["tasks"]) for c in list_components(Project(s))], [("dashboard", 2)])

    def test_edit_and_assign_by_files(self):
        s = MemoryStore(tasks=[task("T-1", files=["dash/a.ts"])], meta={"components": [{"id": "dashboard", "paths": []}]})
        result = edit_component(Project(s), "dashboard", add_paths=["dash/**"], assign=True)
        self.assertEqual((result["changed"], result["by_files"]), (["paths"], ["T-1"]))
        with self.assertRaisesRegex(AtmFailure, "no component 'nope'"):
            edit_component(Project(s), "nope", label="x")


if __name__ == "__main__":
    unittest.main()
