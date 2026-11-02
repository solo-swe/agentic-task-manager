"""atm-lib's task views, search and claims (lib/query.py), behind the REST API's routes: what atm derives for each
task, so the dashboard doesn't apply atm's rules itself. Each view fits api.schema.json's TaskView."""

import unittest

from schema_check import load_schema, problems
from support import task

from atm.db.memory import MemoryStore
from atm.lib.project import Project
from atm.lib.query import Filters, list_claims, search_tasks, task_views

API = load_schema("api.schema.json")
META = {
    "sprints": [{"id": "One", "state": "active", "release": "1.0"}],
    "releases": [{"id": "1.0", "state": "planned"}],
    "components": [{"id": "ui", "label": "UI", "paths": ["ui/**"], "review": "async"}],
    "review": "batch",
}


def project(tasks, claims=None):
    return Project(MemoryStore(tasks=tasks, meta=META, claims=claims))


def derived(views):
    return {v["task"]["id"]: v["derived"] for v in views}


class TaskViews(unittest.TestCase):
    def setUp(self):
        self.views = task_views(
            project(
                [
                    task("T-1", status="done", sprint="One", pr={"number": 3}, merged_at="2026-01-01T00:00:00Z"),
                    task("T-2", status="done", pr={"number": 4}),
                    task("T-3", dependencies=["T-1"], sprint="One", components=["ui"]),
                    task("T-4", dependencies=["T-3", "T-2"], review="none", release="1.0"),
                    task("T-5", status="blocked", waiting_on="user", priority="P0"),
                ]
            ),
            Filters(),
        )

    def test_every_view_fits_the_schema(self):
        self.assertEqual(problems({"generated_at": "now", "tasks": self.views}, API), [])

    def test_kind_ready_and_what_it_waits_for(self):
        d = derived(self.views)
        self.assertEqual((d["T-3"]["kind"], d["T-3"]["ready"], d["T-3"]["waiting_for"]), ("todo", True, []))
        self.assertEqual((d["T-4"]["ready"], d["T-4"]["waiting_for"]), (False, ["T-3"]))
        self.assertEqual((d["T-5"]["kind"], d["T-5"]["waiting_on"]), ("blocked", "user"))
        self.assertNotIn("waiting_on", d["T-3"])  # what doesn't apply is left out

    def test_effective_priority_review_and_release(self):
        d = derived(self.views)
        self.assertEqual((d["T-5"]["priority"], d["T-3"]["priority"]), ("P0", "P2"))
        self.assertEqual(d["T-3"]["effective_review"], "async")  # its component's
        self.assertEqual(d["T-4"]["effective_review"], "none")  # its own
        self.assertEqual(d["T-5"]["effective_review"], "batch")  # the project's
        self.assertEqual((d["T-3"]["effective_release"], d["T-4"]["effective_release"]), ("1.0", "1.0"))
        self.assertNotIn("effective_release", d["T-5"])

    def test_the_stages_of_done(self):
        d = derived(self.views)
        self.assertEqual((d["T-1"]["stage"], d["T-2"]["stage"]), ("merged", "in_pr"))
        self.assertNotIn("stage", d["T-3"])

    def test_filtered_like_atm_list(self):
        views = task_views(project([task("T-1", sprint="One"), task("T-2")]), Filters(backlog=True))
        self.assertEqual([v["task"]["id"] for v in views], ["T-2"])


class Search(unittest.TestCase):
    def test_every_word_in_the_task_or_the_project_name(self):
        tasks = [task("T-1", name="Fix the login", labels=["auth"]), task("T-2", notes=[{"text": "login works"}])]
        p = project(tasks)
        self.assertEqual(search_tasks(p, "LOGIN"), ["T-1", "T-2"])
        self.assertEqual(search_tasks(p, "login auth"), ["T-1"])
        self.assertEqual(search_tasks(p, "acme login", also="Acme"), ["T-1", "T-2"])
        self.assertEqual(search_tasks(p, "  "), [])


class Claims(unittest.TestCase):
    def test_by_task(self):
        claims = {"claims": {"T-2": {"task": "T-2", "agent": "b"}, "T-1": {"task": "T-1", "agent": "a"}, "x": "bad"}}
        self.assertEqual([c["agent"] for c in list_claims(project([], claims))], ["a", "b"])


if __name__ == "__main__":
    unittest.main()
