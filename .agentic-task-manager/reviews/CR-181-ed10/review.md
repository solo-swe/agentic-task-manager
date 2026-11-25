+++
id = "01a20feb-50b8-7273-a682-3a2b4533ed10"
number = 181
title = "Inbox: another project's item's status, with its tasks' statuses once injected"
status = "approved"
tasks = ["ATM-773"]
author = "claude:33cba067"
reviewers = []
created = 2026-11-24T23:36:49Z
updated = 2026-11-24T23:44:06Z
+++

The agent that left a note in another project's inbox (ATM-772) can check what became of it. No new API: the same `inbox::show` and `inbox::list` the route and the app call.

- **`task_states`** (`inbox.rs` `with_task_states`): an injected item's answer adds what each of its tasks is now, beside `tasks` (kept as their keys, as stored): `{key, found}` and, found, `name`, `status`, `kind`, `archived`. A task of this project is read from its store, the archive too (an archived task is done, unless skipped); a key of another registered project's task as ADR-007 §7 reads one (`foreign::Lookup`), with its `project`. The dashboard's answer, the page and `atm inbox show` all get it from there.
- **`atm inbox show IN-3 --project <key or ID>`**, or `KEY:IN-3`: the item as that project has it, its tasks one a line with their statuses. Both given and different: refused.
- **`atm inbox list --project … --mine`**: the items this agent left there from this project (`inbox::added_by`: by `created_by` and `from_project`; without `--project`, the project's own items it added). The counts stay the whole inbox's.
- **The page** (`InboxPage.svelte`): each injected task's status beside its key, by the project's label, *not found* when atm can't find it; an answer without states shows the keys as before.
- **Docs:** `docs/agent/commands.md` (how to check, and that an item's tasks are that project's work, never to take or change from here) and `docs/security-architecture.md` (the read entry points, read only, registered projects only).

**Tests:** atm-lib: each kind of task (active, todo, archived done, archived skipped, not found) on `show` and `list`, items with no tasks untouched; `added_by`; a third project's task through a registered project, and as it changes there. `InboxPage`'s component test (the statuses, *not found*, none without states). The CLI suite: show with `--project` and as `KEY:IN-1 --json`, the two together refused, `list --project --mine`, an unregistered project refused. Clippy, the CLI's scenarios, the unit tests and the functional e2e suite pass.

The route needed no change: it answers what `inbox::list` gives (its tests use a fake atm-lib, so they don't see the field).
