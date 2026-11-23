+++
id = "01a1f197-5401-7507-8d8a-62838fad5e26"
number = 46
title = "The dashboard shows keys plain; a moved task says the key it had"
status = "approved"
tasks = ["ATM-637"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T02:16:28Z
updated = 2026-11-19T05:27:40Z
+++

The dashboard and the app show every task key plain (ADR-009).

## Change
- **The draft mark goes:** the lighter italics, the ⓘ and its tooltip.
  - `keys.ts` loses `isProvisional`, `PROVISIONAL_TIP` and `RefView.provisional`.
  - `TaskKey.svelte` is plain text.
  - `DepGraph`'s node label has no italic tspan.
  - `waiting.ts` and `ProjectCard` no longer carry `key_final`; the generated types no longer have it (ATM-635's schemas).
- **A task a merge moved says so:** its details show **"was ATM-9"** under the title, from the new `movedFrom()`: its aliases under the project's current key. An old ID (`T-P1-03`) or another prefix isn't a move. `?task=ATM-9` opens it.
- **Old texts:** a key written with an old `~` (`~ATM-10`) still finds its task (`findTask`), in addresses and in links in task text.

## Tests
- **Unit (`keys.test.ts`):** keys plain; `movedFrom` (own-prefix aliases only, any case, a key matched as text, none without a project key); `findTask` with `~`; `refView` without `provisional`. `ProjectCard`: plain keys, no mark.
- **e2e (`keys.spec.ts`):**
  - plain keys on the graph, Kanban, list, details, overview and All projects board, with no mark anywhere;
  - the moved task's details say "was ATM-9" (not its old ID), and an unmoved task's say nothing;
  - `?task=ATM-9` and `?task=~ATM-10` open their tasks.
- **Suites:** checks, the unit tests and the full functional e2e run pass. That run covers ATM-635's API change too.
- **Screenshots** that showed a draft mark are regenerated in the release task.
