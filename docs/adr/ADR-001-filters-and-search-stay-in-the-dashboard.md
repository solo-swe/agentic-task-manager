# ADR-001: Filtering and search stay in the dashboard

- **Status:** accepted, 2026-11-06
- **Decided by:** Hakan Serce
- **Context:** T-435, [dashboard-logic-inventory.md](../dashboard-logic-inventory.md) §2 (`filters.ts`)

## Context

The [Svelte guide](../guides/svelte.md#where-logic-lives) puts validation, queries, ordering and search in atm-lib,
served through the v1 API, and keeps the dashboard to showing data and sending intent. The inventory of what the
dashboard computes (T-435) found that filtering and search are the one place where following the rule now would cost
the user something.

Today `dashboard/src/lib/filters.ts` filters, counts, sorts and searches the tasks the dashboard has already loaded:
the Kanban, List and Graph react to a click at once. Moving that to atm-lib, as the v1 tasks route's filters
(`atm list`'s) would allow, means:

- **A round trip per click.** The server reads and parses `tasks.json` (about 1 MB in this project) for every
  filtered request. The e2e interaction budget is 200 ms locally and 400 ms in CI (`tests/e2e/performance.spec.ts`).
- **The counts beside each filter value** are over every task, not the filtered ones: the server would have to send
  them too, a new kind of answer.
- **The archive and the worktrees' copies** are filtered with the rest today (Include archived, the worktree board);
  each would need the same filtering on the server.
- **Search would cover less.** `search_tasks` (atm-lib, `api/search`) skips the archive, which the dashboard's search
  covers when archived tasks are shown.
- **The e2e fixture** that plays the server (`tasksServer`) would have to filter as atm does: atm's rules in
  TypeScript a second time, or the real server under the e2e suite.

## Decision

Filtering, the counts beside the filter values, sorting and search **stay in the dashboard**, over the tasks it has
loaded, as they are now.

This holds **until there is a server-side design that handles them properly**: one that keeps a filter click as fast
as now (within the e2e interaction budget), answers the counts, covers the archive and the worktrees' copies, and
lets the e2e suite test it without re-implementing atm's rules. Such a design gets its own ADR, which replaces this
one.

Until then:

- What a task's values *are* is atm's wherever atm sends it: the dashboard compares the `derived` values the v1 route
  gives (readiness, effective release, review mode, and those the inventory's step 1 adds), rather than working them
  out again. Only matching, counting, sorting and searching the loaded list stay in `filters.ts`.
- New filters follow the same split: the value comes from atm, the matching is the dashboard's.
- The Svelte guide's rule otherwise applies: this is the one exception, and the guide names it.

## Consequences

- Filtering stays instant, and works the same on the archive and the worktree copies.
- Two definitions of "matches" exist: `atm list`'s filters and search in atm-lib, and `filters.ts` in the dashboard.
  They must agree on what a value means; the derived values keep that in one place, and a disagreement found later
  is a bug in `filters.ts`.
- The inventory's step 5 (filters and search to the server) is deferred, not planned.
