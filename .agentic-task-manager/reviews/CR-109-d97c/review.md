+++
id = "01a1febe-c88f-73a1-a13a-84f20551d97c"
number = 109
title = "The dashboard’s own rules, and moving them behind atm’s API"
status = "approved"
tasks = ["ATM-716"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-21T15:34:38Z
updated = 2026-11-21T15:49:36Z
+++

The scan you asked for after ATM-711's card ("Approved", and under it "Waiting for review · 1 h 21 m"): every place the dashboard decides what task, review, sprint, claim or project data means instead of showing what atm computed, and the plan to move each rule into atm-lib behind the routes and the app's commands.

**What it found** (§3, eight groups A–H, each place with its file and line, what atm has today, and a severity):
- about 40 places work out a status's kind, though every task comes with `derived.kind`;
- a second vocabulary parser, a second sprints model, a second release-notes writer;
- the work-time rule four times in the dashboard, and once more in atm's summary, which copied it;
- the review page has atm's `can`, but uses it for two of its buttons.

**Wrong today, with an example each:**
1. The waiting line ignores the review's decision (ATM-711). The same mistake makes the forecast, the burndown, the component table and the "waiting on you" time count approved or sent-back tasks as not needing agent work.
2. An empty sprint is 0 % in the scope bar and 100 % on the overview card.
3. The Project tab's backlog count includes finished tasks; `atm sprint list` doesn't.
4. "With you" is the `interactive` label in the charts and the `with: user` field elsewhere.
5. A due date passes by the viewer's local day in one place and by UTC in another.
6. Archived skipped tasks count as done until the archive loads.
7. The overview's Progress tab recomputes from raw tasks what the cards beside it take from atm.

**The proposal** (§4):
- new `derived` fields: `waiting`, `needs_you`, `agent_work`, `review`, `work`, reverse links, `in_closed_sprint`;
- four project routes: vocabulary, sprints, releases, stats;
- `can` for every review action;
- the claims and worktrees decided by atm;
- a guard test so the dashboard doesn't grow rules again.

**The order** (§5): eight steps, waiting and needs-you first (P0). After your approval each becomes a task in this sprint.

**Four questions for you** (§6): "with you"; the forecast in atm; what a broken vocabulary shows; work over several attempts.
