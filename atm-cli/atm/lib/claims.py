"""atm-lib: claims and the loop's next task (docs/layered-architecture.md §5.1): `atm next` (with `--claim`, atomic),
heartbeats, the live claims. Each runs as one change of the store (§3.3); the loop's rule itself (`next_task`,
`Selection`) is still claims.py's, from before the layers, and moves with a later slice."""

from datetime import timedelta

from ..claims import (
    Selection,
    apply_resolutions,
    check_holder,
    expire_claims,
    lease_minutes,
    lease_until,
    next_task,
    own_lease_expired,
)
from ..common.errors import Claimed
from ..core import now, now_iso, parse_iso
from ..model import change_status
from .status import claim_record

# heartbeat --auto renews a claim once its last renewal is this old (CLI-6.2)
AUTO_RENEW_AFTER = timedelta(minutes=10)


def selection(sprints=None, tasks=None, all_tasks=False, backlog=False, task_type=None):
    """What the loop works on: the active sprint by default, named sprints, a list of tasks, all, or the backlog."""
    return Selection(sprints=sprints, tasks=tasks, all_tasks=all_tasks, backlog=backlog, task_type=task_type)


def next_task_for(project, sel, with_user=False, claim=False, status=None):
    """The task the loop takes next for `project.actor` (the dashboard's Next up is the same rule); with `claim`,
    started and claimed in the same change, so no other agent can take it. Returns {task, claim, reason, expired}:
    `reason` says why there is none."""
    agent = project.actor

    def change(work):
        data, store = work.document, work.claims()
        expired = expire_claims(None, data, store["claims"])
        apply_resolutions({"root": work.here()["worktree"]}, data, store)
        task, reason = next_task(data, store, sel=sel, agent=agent, with_user=with_user)
        new = None
        if task and claim:
            change_status(task, data, "start", agent=agent, status=status)
            new = store["claims"][task["id"]] = claim_record(work, data, task["id"], agent)
        return {"task": task, "claim": new, "reason": reason, "expired": expired}

    return project.store.change(change)


def renew_lease(project, task_id, expire=True):
    """Extend the actor's lease on a task (`atm heartbeat`; `atm run` with `expire` false: it renews a live claim and
    lets nothing else expire). Returns {claim, minutes, expired}; raises Claimed when the task has no live claim (its
    lease expired) or another agent holds it."""
    agent = project.actor

    def change(work):
        data, claims = work.document, work.claims()["claims"]
        if not expire:
            claim = claims.get(task_id)
            try:
                alive = claim is not None and parse_iso(claim["lease_expires_at"]) > now()
            except (KeyError, TypeError, ValueError):
                alive = False
            if not alive:
                raise Claimed(f"{task_id} has no live claim")
            check_holder(claims, task_id, agent, "renew the claim on")
            claim["lease_expires_at"] = lease_until(data)
            claim["renewed_at"] = now_iso()
            return {"claim": claim, "minutes": lease_minutes(data), "expired": []}
        expired = expire_claims(None, data, claims)
        claim = claims.get(task_id)
        if not claim:
            own = own_lease_expired(expired, data, task_id, agent, None)
            gone = any(c.get("task") == task_id for c in expired)
            work.defer(own or Claimed(f"{task_id} has no live claim" + (" (its lease expired)" if gone else "")))
            return None
        check_holder(claims, task_id, agent, "heartbeat")
        claim["lease_expires_at"] = lease_until(data)
        claim["renewed_at"] = now_iso()
        return {"claim": claim, "minutes": lease_minutes(data), "expired": expired}

    return project.store.change(change)


def renewed_at(claim):
    try:
        return parse_iso(claim.get("renewed_at") or claim["claimed_at"])
    except (KeyError, TypeError, ValueError):
        return None


def renew_due_claims(project):
    """`heartbeat --auto`: renew the actor's live claims whose last renewal is ten minutes old. Most calls only read
    the claims (no change, nothing written). Returns the claims renewed."""
    agent, current = project.actor, now()

    def due(claims):
        return [
            c
            for c in claims.values()
            if isinstance(c, dict)
            and c.get("agent") == agent
            and parse_iso(c["lease_expires_at"]) > current
            and (renewed_at(c) is None or current - renewed_at(c) >= AUTO_RENEW_AFTER)
        ]

    if not due(project.store.read_claims()["claims"]):
        return []

    def change(work):
        renewed = due(work.claims()["claims"])
        for claim in renewed:
            claim["lease_expires_at"] = lease_until(work.document)
            claim["renewed_at"] = now_iso()
        return renewed

    return project.store.change(change)


def live_claims(project):
    """The live claims, by task, with the expired ones dropped (their tasks back to todo). Returns {claims, expired}."""

    def change(work):
        claims = work.claims()["claims"]
        expired = expire_claims(None, work.document, claims)
        return {"claims": sorted(claims.values(), key=lambda c: c["task"]), "expired": expired}

    return project.store.change(change)
