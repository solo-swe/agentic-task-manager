+++
id = "01a20fce-af30-7290-8e4d-ec481a5d143e"
number = 178
title = "Inbox: an agent adds an item to another project's inbox"
status = "approved"
tasks = ["ATM-772"]
author = "claude:33cba067"
reviewers = []
created = 2026-11-24T23:05:33Z
updated = 2026-11-24T23:22:48Z
+++

An agent working in one project can leave a finding in **another** registered project's inbox, for that project's user to triage. No new API: the same `inbox::add`, reachable for another project.

- **`atm inbox add --project <key or ID>`** (`crates/atm-cli/src/commands/inbox.rs`): finds the project among the user's registered ones (`foreign::registered_project`, by key in any case or `meta.id`, never the project it runs in, never a path), and adds through `inbox::add` with this project as its origin. An unknown name is refused naming the registered ones; a key two projects share, until named by ID. `--json` answers the item with its project.
- **Who** (`inbox::add`, the one rule): to its own project's inbox a person only, as before; from another project an agent too, recorded as `from_project` (the adding project's key, else its ID). The page's route and the app's command pass no origin: the user adding to the project shown.
- **The page** shows *from LFB* beside the item's author (`InboxPage.svelte`); the schema has `from_project`.
- **Docs:** `docs/agent/commands.md` (write a finding for its user, never instructions for its agents), the inbox design's §3 table, and `docs/security-architecture.md` (I1's note and the ATM-772 entry: which project, what it writes, that it is data).

**Tests:** atm-lib (an agent of another project adds with its origin, its own refused, a person as before, the text limits with an origin; a project found by key or ID, never the one asking, unknown and ambiguous names), the component test (*from …*), and the CLI suite's `inbox add --project` scenarios with two registered projects.

The work was started by an earlier session (its lease lapsed); finished and verified here: atm-lib's inbox and foreign tests, the CLI suite, clippy, the dashboard's check and unit tests.
