+++
id = "01a1f086-ca17-73c5-b779-8e6c5c446fee"
number = 26
title = "A direct push makes task keys final"
status = "approved"
tasks = ["ATM-623"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T21:18:47Z
updated = 2026-11-18T21:23:16Z
+++

Task keys now become final after a direct push to the base branch, not only when a merged PR is recorded.

**Why they stayed provisional:** ADR-007 §3 says `atm vcs pushed` (*a PR merged, or a direct push*) and `atm db migrate` make keys final. But `atm vcs pushed` required `--pr`, and `/atm-push` never called it in a `direct` project. So a project working on `main` got final keys only at the next `atm db migrate`, and never if pushes were made by hand without a skill.

**What changed:**
- **`atm vcs pushed` without `--pr`** records a direct push. It makes final the keys of the tasks now on `origin/<base>` (read from git, as always), and says which (`Final now (on the base branch): FIG-1`, or `No provisional key is on the base branch yet.`). With `--json`: `{"keys_final": [...]}`. `--at` without `--pr` is refused (it is a PR's merge time).
- **`/atm-push`**, Step 3: in a `direct` project, after the push, run `atm vcs pushed`, and commit the change on its own if any key became final (it goes with the next push). It is already pre-approved (`atm vcs pushed:*`).
- **`atm docs commands`** lists the new form.

**Checked, as decided with you:**
- What atm stores in the task file links by UUID: `dependencies`, `related`, `duplicate_of`, claims, notes (ADR-007 §1, `edit.rs`).
- **One exception:** a review's tasks are stored by key in `review.md` (ATM-594, so the file reads well). A renumbered provisional key keeps working through its alias. But if another task later takes the old number, the review names that task instead. That's worth a follow-up: store the UUID in `review.md` and show the key. Shall I add it?

**Not changed, a question for you (ADR-007):** should the `~` be hidden where a collision can't happen (one clone, working on its base branch)? Today it shows until the push and `atm vcs pushed`.

Which project was it? If it pushes without `/atm-push`, the keys still wait for the next `atm db migrate` (or a manual `atm vcs pushed`).

**Tests:**
- CLI suite: a direct push makes a pushed task's key final and leaves an unpushed one provisional, then nothing more on a second run. Also the dry run, and `--at` without `--pr` refused.
- The whole CLI suite passes (822 tests). Clippy and fmt pass.

The skill changed, so the security checklist's written skill review is needed before the release PR.
