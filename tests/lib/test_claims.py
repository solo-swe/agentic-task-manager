"""atm-lib's claims and next task (lib/claims.py) on the in-memory store."""

import unittest

from support import task

from atm.common.errors import AtmFailure
from atm.db.memory import MemoryStore
from atm.lib.claims import live_claims, next_task_for, renew_due_claims, renew_lease, selection
from atm.lib.project import Project

PAST, FUTURE = "2000-01-01T00:00:00Z", "2999-01-01T00:00:00Z"


def claim(task_id, agent, until=FUTURE, renewed=PAST):
    return {"task": task_id, "agent": agent, "claimed_at": renewed, "lease_expires_at": until}


class NextTask(unittest.TestCase):
    def test_the_most_urgent_ready_task_then_the_lowest_id(self):
        s = MemoryStore(tasks=[task("T-1", dependencies=["T-9"]), task("T-2"), task("T-3", priority="P0")])
        result = next_task_for(Project(s, actor="a"), selection())
        self.assertEqual((result["task"]["id"], result["claim"]), ("T-3", None))
        self.assertEqual(s.tasks[2]["status"], "todo")  # only looked at

    def test_claim_starts_it_in_the_same_change(self):
        s = MemoryStore(tasks=[task("T-1")])
        result = next_task_for(Project(s, actor="a"), selection(), claim=True)
        self.assertEqual((s.tasks[0]["status"], s.claims["claims"]["T-1"]["agent"]), ("in_progress", "a"))
        self.assertEqual(result["claim"]["task"], "T-1")
        # A second agent gets nothing: the only task is claimed
        other = next_task_for(Project(s, actor="b"), selection(), claim=True)
        self.assertIsNone(other["task"])
        self.assertIn("nothing ready", other["reason"])

    def test_work_with_the_user_waits_for_them(self):
        s = MemoryStore(tasks=[task("T-1", **{"with": "user"})])
        self.assertIsNone(next_task_for(Project(s, actor="a"), selection())["task"])
        self.assertEqual(next_task_for(Project(s, actor="a"), selection(), with_user=True)["task"]["id"], "T-1")

    def test_an_expired_lease_frees_the_task_first(self):
        s = MemoryStore(tasks=[task("T-1", status="in_progress")], claims={"claims": {"T-1": claim("T-1", "a", PAST)}})
        result = next_task_for(Project(s, actor="b"), selection(), claim=True)
        self.assertEqual(result["task"]["id"], "T-1")
        self.assertEqual([c["agent"] for c in result["expired"]], ["a"])


class Leases(unittest.TestCase):
    def test_heartbeat_extends_the_holders_lease(self):
        s = MemoryStore(tasks=[task("T-1", status="in_progress")], claims={"claims": {"T-1": claim("T-1", "a")}})
        result = renew_lease(Project(s, actor="a"), "T-1")
        self.assertIn("renewed_at", s.claims["claims"]["T-1"])
        self.assertGreater(result["minutes"], 0)

    def test_another_agents_claim_or_none_is_refused(self):
        s = MemoryStore(tasks=[task("T-1", status="in_progress")], claims={"claims": {"T-1": claim("T-1", "a")}})
        with self.assertRaises(AtmFailure) as refused:
            renew_lease(Project(s, actor="b"), "T-1")
        self.assertEqual(refused.exception.code, 4)
        with self.assertRaisesRegex(AtmFailure, "no live claim"):
            renew_lease(Project(s, actor="a"), "T-9", expire=False)

    def test_auto_renews_only_what_was_renewed_ten_minutes_ago(self):
        claims = {"claims": {"T-1": claim("T-1", "a"), "T-2": claim("T-2", "a", renewed="2999-01-01T00:00:00Z")}}
        s = MemoryStore(tasks=[task("T-1"), task("T-2")], claims=claims)
        renewed = renew_due_claims(Project(s, actor="a"))
        self.assertEqual([c["task"] for c in renewed], ["T-1"])
        self.assertEqual(renew_due_claims(Project(s, actor="b")), [])

    def test_the_live_claims_without_the_expired(self):
        claims = {"claims": {"T-2": claim("T-2", "b"), "T-1": claim("T-1", "a", PAST)}}
        s = MemoryStore(tasks=[task("T-1", status="in_progress"), task("T-2", status="in_progress")], claims=claims)
        result = live_claims(Project(s))
        self.assertEqual([c["task"] for c in result["claims"]], ["T-2"])
        self.assertEqual(s.tasks[0]["status"], "todo")  # its lease ran out


if __name__ == "__main__":
    unittest.main()
