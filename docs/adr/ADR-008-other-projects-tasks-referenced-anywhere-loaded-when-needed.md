# ADR-008: Other projects' tasks: referenced from any registered project, loaded when needed, never watched

- **Status:** accepted, 2026-11-16 (T-552); replaces the lookup and the freshness of
  [ADR-007](ADR-007-task-keys-uuids-and-dependencies-across-projects.md) §7 (the rest of ADR-007 stands)
- **Decided by:** Hakan Serce, in the review of
  [cross-project-freshness-design.md](../cross-project-freshness-design.md) (three rounds in Crit, 2026-11-16):
  - "we can reference any project, but we don't need to watch them all ... load them when we need them";
  - "it is quite acceptable to have the website refresh every few seconds while the app gives the more premium
    experience";
  - watching for changes "should be hidden behind the db abstraction"
- **Context:**
  - [the freshness design](../cross-project-freshness-design.md): its use cases (§4), options (§5) and decisions (§9);
  - [ADR-003](ADR-003-workspaces.md), workspaces;
  - [layered-architecture.md](../layered-architecture.md), atm-db as the store abstraction;
  - the tasks that built it: T-544, T-549 and T-506; T-507, reverted.

## Problem

ADR-007 §7 decides how a task depends on another project's task (`<project id>:<UUID>`, written by key, resolved
through the registry), but leaves three things open or out of step with the code:

- **Where a key is looked up.** §7 says among "the projects of the workspaces it is listed in", the referring
  project's own workspaces. The code (`foreign::registered`) looks among every registered project, in every
  workspace.
- **How the other project stays up to date.** The rules cached its tasks' status for the life of the process, so in
  `atm serve` and the app a task kept waiting on a task long done. The browser's ETag didn't cover the other project
  either, so its reload kept the old status behind a 304 (design §4, P1 to P4).
- **Who watches for changes, and how.** The watcher (`notify`) sat above the store, on folders the store reported, so
  it assumed every store keeps files on disk (design §3, P6).

## Decision

1. **Any registered project can be referenced.** A dependency, a related task or a duplicate may name a task in any
   project of any workspace the user registered. A key (`ATM-978`) is looked up among all of them. Two projects with
   the same key stay as ADR-007 §8 says: refused by key, named by UUID.
2. **Another project is loaded when it is needed, and never watched.**
   - **When it is read:** when something needs it, that is deciding whether a task is ready (`atm loop next`, the
     views' `ready` and `waiting_for`, the summary's `next`) and showing it (the details panel, the overview card).
   - **The cache:** one cache serves both. It is keyed by the other project's store `stamp()` and expires after a few
     seconds, so a read is cheap when nothing changed, and never stale for long when something did (design §5.1 B).
3. **What is watched is the open workspace's own projects, and only for the app.** The app is pushed changes. A change
   in another project shows on the app's next reload: a change in the open workspace, the window's focus, or Reload.
4. **The browser polls; the app is pushed.**
   - The browser reloads at the interval of its refresh settings, as before T-507, whose change stream is reverted.
   - The tasks route's ETag also covers the stamps of the projects the project's tasks reference, so a poll sees
     their changes and still gets a 304 for anything else (design §5.3 D).
5. **How a store hears of changes is the store's.**
   - **The trait:** atm-db's `Store` has `subscribe(on_change)`, in place of `watch_folders()`. Push or poll is
     each implementation's choice, hidden behind it: the JSON store watches its files, and a store that can't push
     polls (atm-db offers a `poll_stamp` helper).
   - **atm-lib:** merges a workspace's subscriptions. Nothing above atm-db names a folder to watch.
6. **Another project counts as its main working copy.** A task done only in one of its worktrees' unmerged branches is
   not done for a task that depends on it. Showing "done in `<branch>`, not merged" in the display may come later
   (design §4.2, V2).

## Alternatives considered

- **References only within the referring project's own workspaces** (ADR-007 §7 as written, design K3). Rejected:
  Hakan wants any project referable; the code already allowed it.
- **Watching every referenced project** (design H, I). Rejected: loading when needed, with a stamp-keyed cache, is
  as fresh at the moment that matters, and watches nothing it doesn't show live.
- **The browser pushed too** (design 2; T-507's server-sent events). Rejected: the browser a few seconds behind is
  acceptable. The stream brought long-lived connections, their limit and reconnects, a server-side watcher per tab,
  and an entry point, and still needed the ETag fix.
- **A per-process cache cleared on change messages** (design C). Rejected: only as good as the watch, and a polling
  browser has no messages.
- **The ETag covering every registered project** (design E). Kept as the fallback: it would re-read the open project
  on every change anywhere.

## Risks

- **Coarse timestamps:** a write within the file system's time resolution that keeps the file's size looks unchanged
  to the stamp. Nanosecond times on APFS and ext4 make it unlikely; the cache's expiry of a few seconds bounds it
  elsewhere.
- **The app's view of another project** can lag until its next reload (decision 3), while the CLI is always fresh.
  Acceptable: what agents act on (`atm loop next`) reads fresh.
- **A store's subscription runs on its own thread:** the trait's documentation says so, and atm-lib holds no lock
  across it.

## Consequences

- **The tasks that build it:**
  - T-553: the cache;
  - T-554: the ETag;
  - T-555: `Store::subscribe` in atm-db;
  - the revert of T-507: the browser polls again, with its refresh settings.
- **ADR-007 §7** points here for the lookup and the freshness. Its form for a reference (`<project id>:<UUID>`) and
  its security rule (only registered projects, read only, through the store's checks) stand.
- **docs/security-architecture.md:** the cross-project entry point reads every registered project, as before; the
  change stream's entry point is gone with T-507's revert.
