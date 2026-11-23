+++
id = "01a1f0cb-3a42-77ae-9d1b-658360037f99"
number = 30
title = "ADR-009: task keys plain from creation"
status = "approved"
tasks = ["ATM-634"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T22:33:32Z
updated = 2026-11-19T01:22:23Z
+++

ADR-009 records what we decided: no provisional keys. Keys are plain from creation. A number clash at a merge moves one task to the next free number, and its old key stays as an alias.

## One point to check: which task moves

"The incoming side's task moves" depends on the direction of the merge. Merging main *into* a branch (`git pull`) would move a task that is already on main, and its new number would then come back to main with the branch. ADR-007's `key_final` flag prevented that.

Without a stored flag, §3 proposes:
- **The task already on the base branch keeps its number.** atm checks this from git when the merge runs and stores nothing.
- **Otherwise the newer task moves.** UUIDv7 sorts by creation, so both merge directions and every clone agree.

Is that the rule you want?

## Also
- **§1:** what's kept from ADR-007.
- **§4:** the migration (`key_final` dropped once).
- **Alternatives:** the options we discussed, one line each.
- **ADR-007:** its status line points here.

Once you approve, ATM-635, ATM-636 and ATM-637 start. I'll update ATM-636's description to this rule.
