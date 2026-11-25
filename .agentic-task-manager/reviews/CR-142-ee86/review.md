+++
id = "01a20b4f-9bae-75c7-8ac0-6761c0f3ee86"
number = 142
title = "Inbox: the store, the merge and the rules"
status = "approved"
tasks = ["ATM-759"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T02:08:16Z
updated = 2026-11-24T02:40:51Z
+++

The inbox's store and rules (`docs/inbox-design.md` §2–§4 and §8), below the front ends. The commands, routes and page come in ATM-760–762.

**What changed:**
- **atm-common:**
  - `inbox.rs`: the files' names, the `IN-<n>` IDs, the limits (text up to 20,000 characters, a reason up to 500), the states, the document check, and the next number.
  - `schema/inbox.schema.json`: the inbox, from which the dashboard's types are generated.
  - `task.schema.json`: a task's `from`, naming the items it was made from.
- **atm-db:**
  - `Working` gets the inbox and its archive, the way it already has the task archive: read on first use, written only when changed, under the store's one lock, together with the tasks. `with_inbox` keeps the constructor unchanged.
  - `JsonGitStore` reads and writes `inbox.json` and `inbox-archive.json`. The archive is written first when items move into it. Once there is an inbox, `.gitattributes` gets its merge lines.
  - The task folder's symlink check covers both files (I6), and `Store::inbox` gives the commands that only read a way in.
- **The merge** (`inbox_merge.rs`, dispatched by the file name in `merge_versions`):
  - Items are merged by ID, and an injection is final.
  - An item dropped on one branch stays dropped unless the other changed it.
  - A number both branches gave to different items: ours keeps it, theirs takes the next free one and records the old one as `renumbered_from`. As with the tasks' renumbering, that isn't a conflict, so the merge goes on.
- **atm-lib `inbox.rs`:** `add` (a person only), `edit` (a person, open items), `discard` (with a reason), `inject` (into working tasks, setting each task's `from` in the same change; final), `restore` (a person, discarded items), plus `list`, `show`, `archive` (decided items, by `keep_days`), `unarchive` and `open_count`.
- **The security doc:** I6 and I9 now name the inbox's files.

**A bug the tests caught:** a new item could take the number of an archived item. The next number now looks past the archive too.

**Please check:**
- Whether `renumbered_from` without a conflict is the right call for an item numbered on both branches.
- The tests:
  - atm-lib: every move, from every state, by a person and by an agent, each refusal, the limits, archiving and unarchiving, numbering.
  - atm-db: `Working`, the store's writes, an unreadable inbox, the symlink check, the merge.
  - `merge.test.mjs`: the driver given each file's path, and a real git merge of two branches that both added items.
