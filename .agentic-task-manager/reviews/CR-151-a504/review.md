+++
id = "01a20c69-1bdc-748e-994d-eba3067da504"
number = 151
title = "An agent's review is a code review: refuse in_review without one, warn of a stranded task"
status = "approved"
tasks = ["ATM-770"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T07:15:44Z
updated = 2026-11-24T16:39:37Z
+++

The state-machine fix you asked for after ATM-728 got stuck (ATM-770): an agent's review is a code review.

**What happened on ATM-728:** all its CRs were approved, then I asked for one more review with a plain `atm task state ATM-728 in_review`. That opens no CR, and the dashboard decides a task only through a CR, so it waited on a decision you couldn't make there.

**The fix (atm-lib):**
- **`change_task_status`:** an **agent's** move to review is refused when no code review that is **waiting, live or sent back** names the task. The refusal names the way: `atm review create --task <key> …`, or `atm review round CR-N` for a review sent back. Drafts, approved and closed reviews don't count. A project without a folder (no reviews possible) isn't refused.
- **The reviews' own moves stay as they were:** a review opened, a round, a live review's approval. They go through a new `move_task_status`, which skips the rule (the live approval is the agent's move after its review was already approved, so the rule would have refused it).
- **A person's moves and decisions are unchanged:** a person may still ask for a review themselves and decide it.
- **`atm doctor`** warns of a task in review, undecided, that no review waits on, with both ways out. It skips a task the "approved but the task still waits" warning already names.
- **The done refusal:** when an agent finishes a `blocker` or `async` task, the refusal now says to open its code review, not to run `in_review`.

**Skills and docs (please look):** `atm-do-task` and `atm-loop-tasks` told agents to use plain `in_review` for "work with no change to review (a decision, a check)". That's now refused, so they **block on the user with a question** instead. A check in the preview is a review of its change (`--range`). `docs/agent/states.md` and `reviews.md` say the rule. The security doc has a written skill review for it (no sandbox run, as agreed). Is blocking the right answer for a decision task, or would you rather such work open a review of a note file?

**Tests:**
- atm-lib integration (`tests/reviews/asking.rs`): an agent refused with no review; a review of another task, an approved one or a closed one doesn't count; a review opened and a round after changes asked for still work; a person may ask; a project without a folder isn't refused; the doctor warning, stranded and not.
- CLI scenarios (`review-request.json`): the refusal and its message, the done message, and a person's ask then doctor's warning.
- The scenarios that set a task in review through an agent's plain `in_review` now open a review instead. The done message's expectations are updated.
