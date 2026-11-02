"""The store's contract (docs/layered-architecture.md §3): the same checks on every store, so atm-lib can't tell them
apart. Each slice of the migration adds the checks for the operations it adds."""

import json
import os
import shutil
import tempfile
import unittest

from support import task

from atm.db.jsongit.store import JsonGitStore
from atm.db.memory import MemoryStore
from atm.paths import find_paths

TASKS = [task("T-1", status="done"), task("T-2", dependencies=["T-1", "T-0"], priority="P1")]
ARCHIVE = [task("T-0", status="done")]
META = {"project": "Contract"}


class StoreContract:
    """The checks; a subclass gives `make_store()`."""

    def make_store(self):
        raise NotImplementedError

    def setUp(self):
        self.store = self.make_store()

    def test_lists_the_working_tasks_in_order(self):
        self.assertEqual([t["id"] for t in self.store.list_tasks()], ["T-1", "T-2"])

    def test_gets_a_working_task(self):
        self.assertEqual(self.store.get_task("T-2")["priority"], "P1")

    def test_a_working_task_is_not_archived_and_the_other_way_round(self):
        self.assertIsNone(self.store.get_task("T-0"))
        self.assertIsNone(self.store.get_archived_task("T-1"))

    def test_gets_an_archived_task(self):
        self.assertEqual(self.store.get_archived_task("T-0")["status"], "done")

    def test_an_unknown_task_is_none(self):
        self.assertIsNone(self.store.get_task("T-9"))
        self.assertIsNone(self.store.get_archived_task("T-9"))

    def test_the_vocabularies_have_the_defaults(self):
        vocab = self.store.get_vocabularies()
        self.assertEqual([s["kind"] for s in vocab["statuses"]][:2], ["todo", "active"])
        self.assertEqual(vocab["default_priority"], "P2")

    def test_lists_the_archived_tasks(self):
        self.assertEqual([t["id"] for t in self.store.list_archived()], ["T-0"])

    def test_no_claims_yet(self):
        claims = self.store.read_claims()
        self.assertEqual(sorted(claims), ["claims", "completed", "hints", "resolved", "waiting"])
        self.assertEqual(claims["claims"], {})

    def test_sizes_in_bytes(self):
        sizes = self.store.sizes()
        self.assertGreater(sizes["tasks"], 0)
        self.assertGreater(sizes["archive"], 0)

    def test_the_document_has_meta_and_the_working_tasks(self):
        document = self.store.document()
        self.assertEqual(document["meta"]["project"], "Contract")
        self.assertEqual([t["id"] for t in document["tasks"]], ["T-1", "T-2"])


class MemoryStoreContract(StoreContract, unittest.TestCase):
    def make_store(self):
        return MemoryStore(tasks=json.loads(json.dumps(TASKS)), archive=json.loads(json.dumps(ARCHIVE)), meta=META)


class JsonGitStoreContract(StoreContract, unittest.TestCase):
    def make_store(self):
        self.root = tempfile.mkdtemp(prefix="atm-store-")
        self.addCleanup(shutil.rmtree, self.root)
        folder = os.path.join(self.root, ".agentic-task-manager")
        os.makedirs(folder)
        for name, data in (("tasks.json", {"meta": META, "tasks": TASKS}), ("tasks-archive.json", {"meta": {}, "tasks": ARCHIVE})):
            with open(os.path.join(folder, name), "w", encoding="utf-8") as f:
                json.dump(data, f)
        return JsonGitStore(find_paths(self.root))


if __name__ == "__main__":
    unittest.main()
