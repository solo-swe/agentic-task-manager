"""atm-lib's code side (lib/code.py): the workflow, commits, PRs and merges, on the in-memory store with a fake version
control: atm-lib reaches the code's history only through the vcs interface (docs/layered-architecture.md §2)."""

import unittest

from support import task

from atm.common.errors import AtmFailure, Invalid, NotFound
from atm.db.memory import MemoryStore
from atm.lib.code import commit_task, record_commit, record_merge, record_pr, set_workflow, workflow_settings
from atm.lib.project import Project


class FakeVcs:
    """Commits in a list; `history` is what the current branch reaches."""

    def __init__(self, repository=True):
        self.repository = repository
        self.commits = []  # (sha, files, message)
        self.history = {}  # sha -> message

    def in_repository(self):
        return self.repository

    def paths(self, files, cwd):
        return list(files)

    def commit(self, files, message):
        sha = f"{len(self.commits) + 1:040d}"
        self.commits.append((sha, files, message))
        self.history[sha] = message
        return sha

    def resolve(self, rev):
        return rev if rev in self.history else None

    def in_history(self, sha):
        return sha in self.history

    def message(self, sha):
        return self.history[sha]

    def default_base(self):
        return "main"


class StoreWithFiles(MemoryStore):
    def files_for_commit(self):
        return ["tasks.json"]


class Workflow(unittest.TestCase):
    def test_none_until_set_then_its_defaults(self):
        p = Project(MemoryStore(tasks=[]))
        self.assertIsNone(workflow_settings(p))
        workflow = set_workflow(p, "pull-request", base="dev")
        self.assertEqual((workflow["type"], workflow["base"]), ("pull-request", "dev"))
        self.assertEqual(workflow_settings(p)["type"], "pull-request")

    def test_refused_settings_write_nothing(self):
        s = MemoryStore(tasks=[])
        with self.assertRaises(Invalid):
            set_workflow(Project(s), "carrier-pigeon")
        with self.assertRaises(Invalid):
            set_workflow(Project(s), "direct", trailers="require")
        self.assertNotIn("workflow", s.meta)


class Commit(unittest.TestCase):
    def test_commits_the_files_and_the_stores_files_then_records_the_sha(self):
        s, vcs = StoreWithFiles(tasks=[task("T-1", name="Login")]), FakeVcs()
        result = commit_task(Project(s, vcs, "Ada"), "T-1", ["src/a.py"], "/repo")
        sha, files, message = vcs.commits[0]
        self.assertEqual(files, ["src/a.py", "tasks.json"])
        self.assertEqual(message, "feat(T-1): Login")
        self.assertEqual((result["commit"], s.tasks[0]["commit"]), (sha, sha))

    def test_the_workflows_trailer_rule_applies(self):
        s, vcs = MemoryStore(tasks=[task("T-1")]), FakeVcs()
        p = Project(s, vcs)
        set_workflow(p, "direct", trailers="forbid")
        commit_task(p, "T-1", ["a"], "/repo", "fix it\n\nCo-Authored-By: Someone <x@y>")
        self.assertEqual(vcs.commits[0][2], "fix it")

    def test_refused_outside_a_repository_and_without_files(self):
        p = Project(MemoryStore(tasks=[task("T-1")]), FakeVcs(repository=False))
        with self.assertRaisesRegex(AtmFailure, "needs a git repository"):
            commit_task(p, "T-1", ["a"], "/repo")
        with self.assertRaises(Invalid):
            commit_task(Project(MemoryStore(tasks=[task("T-1")]), FakeVcs()), "T-1", [], "/repo")

    def test_record_an_existing_commit_with_a_hint_when_it_doesnt_mention_the_task(self):
        s, vcs = MemoryStore(tasks=[task("T-1"), task("T-2")]), FakeVcs()
        sha = vcs.commit(["a"], "feat(T-1): one")
        p = Project(s, vcs)
        self.assertIsNone(record_commit(p, "T-1", sha)["hint"])
        self.assertIn("doesn't mention T-2", record_commit(p, "T-2", sha)["hint"])
        self.assertEqual(s.tasks[1]["commit"], sha)
        with self.assertRaises(NotFound):
            record_commit(p, "T-1", "nope")


class PullRequests(unittest.TestCase):
    def test_pr_then_merge_releases_the_release_it_completes(self):
        s = MemoryStore(
            tasks=[task("T-1", release="1.0"), task("T-2", release="1.0")],
            meta={"releases": [{"id": "1.0", "state": "planned"}]},
        )
        p = Project(s, actor="Ada")
        record_pr(p, ["T-1"], 7, "https://example.test/pr/7")
        self.assertEqual(record_merge(p, 7, "2026-11-01T00:00:00Z")["pending"], {"1.0": ["T-2"]})
        record_pr(p, ["T-2"], 8, "https://example.test/pr/8")
        s.tasks[0]["pr"] = {"number": 8, "url": "https://example.test/pr/8"}  # both in one PR
        result = record_merge(p, 8, "2026-11-02T00:00:00Z")
        self.assertEqual(result["released"], ["1.0"])
        self.assertEqual(s.meta["releases"][0]["state"], "released")

    def test_a_pr_link_is_only_a_web_address(self):
        p = Project(MemoryStore(tasks=[task("T-1")]))
        with self.assertRaises(Invalid):
            record_pr(p, ["T-1"], 1, "javascript:alert(1)")
        with self.assertRaises(NotFound):
            record_merge(p, 1)


if __name__ == "__main__":
    unittest.main()
