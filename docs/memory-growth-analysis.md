# atm's memory growth: what grows, and what doesn't (ATM-734)

**Status:** analysis, for review with Hakan. Hakan saw "atm" grow from about 70 MB to 4.5 GB after a few hours. This
records what was measured on 2026-11-24 (macOS, the app 0.43.0 from the release, this repository's own project
open in it, agents at work), what is ruled out, and the follow-up work proposed (§5).

## 1. Which process

`footprint` (macOS's physical footprint, what Activity Monitor's Memory column shows) on every atm process, about
two hours after the app started:

| Process | What it is | Footprint | Peak |
|---|---|---|---|
| `com.apple.WebKit.WebContent` (the app's) | the app's page: the dashboard's JS, DOM, layout, images | **427 MB** | 769 MB |
| `atm-app` | the app: Tauri, atm-lib in process, the subscriptions | 105 MB | 160 MB |
| `atm _serve` (Homebrew, 15 days old) | the dashboard server for the browser | 14 MB | 27 MB |
| `atm _serve` (the app's own) | the app's server | 4 MB | 4 MB |
| `atm _claim-watch` × 13 | `atm claim hold`'s watchers (§4) | 6 MB each | |

**The page process is the one that grows.** Its footprint is almost all *WebKit malloc* (288 MB: the DOM, styles,
layout, rendered text) and graphics memory (103 MB), not the JS heap (3 MB in the JS VM's cage). The servers stay
small; the app's own process grows slowly (§2).

## 2. When it grows

The page process and the app, sampled once a minute (`footprint`), while an agent worked in this repository:

| UTC | Page | App | What happened |
|---|---|---|---|
| 00:32 | 427 MB | 105 MB | (the first measurement) |
| 00:38 | 483 MB | 119 MB | e2e runs writing `dashboard/test-results/`, commits, `atm claim hold` writing `claims.json` |
| 00:39 | 498 MB | 120 MB | the same |
| 00:40 | 513 MB | 121 MB | the same |
| 00:41 | 526 MB | 123 MB | the agent stopped changing files |
| 00:42–00:45 | 526 MB | 123 → 106 MB | nothing changed (the app gave memory back) |
| 00:46 | 540 MB | 110 MB | the agent writing this document, holding its claim, an e2e run |

The sampler ran on for the next half hour, while the agent ran e2e suites and committed (each run writes
`dashboard/test-results/`, each commit `tasks.json`):

| UTC | Page | App |
|---|---|---|
| 00:47 | 557 MB | 119 MB |
| 00:51 | 786 MB | 135 MB |
| 00:56 | 906 MB | 115 MB |
| 01:00 | **975 MB** | 136 MB |
| 01:01 | 434 MB | 139 MB (WebKit gave half of it back at once: a memory-pressure purge, or the page loaded again) |
| 01:07 | 601 MB | 143 MB |
| 01:13 | **946 MB** | 131 MB |
| 01:15 | 734 MB | 85 MB |

**It grows while files change, 15 to 40 MB a minute, and not at all while nothing changes**; the app's own process
stays between 85 and 163 MB all along. In the app the page
doesn't poll: it reloads when the app says something changed (`api/changes.ts`: the workspace, the project, and a
live review's working copy). So each reload the app asks for leaves memory behind in WebKit. At an agent's pace
(several changes a minute for hours) that is gigabytes: 15 MB a minute is 4.5 GB in five hours.

## 3. What is ruled out (Chromium)

`dashboard/tests/e2e/memory.spec.ts` (new) measures the page in Chromium through its DevTools protocol: a garbage
collection forced, then the JS heap, the DOM nodes alive (detached ones too) and the event listeners. Each scenario
measures after 10 rounds and again after 50 more:

| Scenario | After 10 | After 60 (or more) |
|---|---|---|
| each task tab (Kanban, List, Graph, Progress, Agents, Project), 60 refreshes with new data | 7.0–7.6 MB, 3,121–4,362 nodes | +0.3 MB, the same nodes and listeners |
| a review's Diffs (three files, a chain on each), read again with a change each time | 7.9 MB, 2,572 nodes, 147 listeners | 9.1 MB, 2,616 nodes, 171 listeners after **160**: it levels off |
| the document viewer, two documents opened and closed 50 times | 10.2 MB, 4,449 nodes | +0.3 MB, the same nodes |

**The dashboard's own state doesn't leak in Chromium**: no DOM node, listener or JS object is kept per refresh.
What grows in the app is WebKit's own memory (*WebKit malloc* and graphics), which Chromium's figures don't show and
WebKit has no protocol to measure from a test. So the cause is in what WebKit keeps per reload: a candidate list,
none confirmed yet:

- **CompMD's rendered pages**: each render of a document or a Markdown diff makes its page (and, for diagrams, a
  sandboxed frame) again; WebKit may keep the frames' documents, or their layers (graphics memory grew too).
- **Layers and images**: the Kanban's and the review page's cards drawn again on each reload, their compositing
  layers or decoded images kept (the *graphics* part of the footprint).
- **WebKit's memory cache** of the IPC answers (each `invoke` answer is a response WebKit may cache).

## 3b. What a reload does in the app (the code, CR-140)

Following one change from the store to the page shows why each reload costs megabytes, and why reloads come every
few seconds while an agent works:

1. **What triggers a reload.** The app's subscriptions (`api/changes.ts`, `+layout.svelte`) watch the project's task
   folder and tell the page on any change in it. The page then reloads **everything** (`dashboard.reload()`: the
   project list, the project's tasks, its claims and worktrees). **Every atm command an agent runs renews its
   lease**, which writes `.agentic-task-manager/claims.json` (`atm docs claims`): an agent at work causes a full
   reload every few seconds, though nothing the page shows has changed but a lease's end.
2. **What a reload carries.** In the browser an unchanged answer is a `304` with no body (the `ETag`s). The app has
   none (`api/ipc.ts`: "No ETags: the app reads its own files"): every reload carries the whole answer again. For this
   repository that is **1.16 MB** of task view (`project_tasks`, 263 tasks with what atm derives) and 52 KB of
   project list, measured from `atm serve`'s same routes.
3. **How many copies.** Each answer crosses the IPC as text (WebKit's copy of the custom-protocol response), is
   parsed by `invoke`, turned back into text by `ipcFetch` (`new Response(JSON.stringify(answer))`) and parsed again
   by the client (`response.json()`): about three copies of every megabyte, the text ones and the `Response`'s
   buffer in WebKit's own memory (*WebKit malloc*), outside the JS heap. They are garbage once parsed, but WebKit
   collects them late: its JS heap stays at a few MB, so nothing presses the collector, while the buffers pile up
   beside it.

At an agent's pace (a dozen atm commands a minute) that is a dozen reloads of 3–4 MB a minute: the 15 to 40 MB a
minute of §2, and the sudden halving at 01:01 is WebKit collecting it at last. Whether all of it is ever given back
(a leak) or only late (garbage) doesn't change the fix: **not making it**. Three bugs, in order of what they save:

- **A lease renewal reloads the page** (P1): the page shows a claim's holder, not its lease's end, so a change of
  `claims.json` that only moves a lease needs no reload; and a change of the claims needs the claims, not the tasks.
  The subscription could say which file changed (`tasks.json`, `claims.json`, `reviews/`), and the page reload only
  that.
- **The app re-sends unchanged answers** (P1): the app's commands could take the stamp the page has (the same one
  `atm serve`'s `ETag` hashes) and answer "unchanged", as a `304` does, so a reload that finds nothing new carries
  nothing.
- **`ipcFetch` turns each answer back into text** (P2): the parsed answer could be handed to the client as it is
  (a `Response`-like object whose `json()` returns it), one copy fewer.

## 4. A second finding: claim watchers that outlive their command

`atm claim hold <id> && <command>` leaves a watcher (`atm _claim-watch`) that renews the claim while the calling
shell lives, at most two hours. An agent's harness keeps one shell for the whole session (Claude Code's does), so
the watcher outlives the command it was for: 13 were alive here after an hour of work, 6 MB each, each renewing the
claim of a task long finished (the renewal of a claim that is gone is refused, so no harm to the tasks). Over a
long loop this is tens of processes and a few hundred MB. Not the 4.5 GB, but its own fix: the watcher should end
with the command, not the shell (§5, follow-up 3).

## 5. Follow-up work proposed

0. **The three bugs of §3b** (P1, P1, P2): reload only what changed and not for a lease, answer "unchanged" in the
   app, one copy fewer in `ipcFetch`. Each with a test: an e2e in the app fixture counting the commands a lease
   renewal makes the page call (none) and the bytes a reload with nothing new carries (none).
1. **Measure the app's page per scenario, in WebKit** (P1): a harness that drives the real app (or Playwright's
   WebKit, sampling its WebContent process with `footprint`) through each reload the app makes (a task change, a
   claim, a working-copy change under a live review, a review comment) and reports the footprint per 100 reloads,
   view by view. That names the view and the kind of reload that leaks.
2. **Fix what it names** (P1), with the harness as its test: a bound on the footprint per 100 reloads.
3. **Claim watchers end with their command** (P2): `atm claim hold` ties the watcher to the command's process (or
   a shell that exits with it) rather than the calling shell, or the watcher ends when the task's claim does.
4. **Fewer reloads in the app** (P2): the page reloads on every change the app reports; changes that don't touch
   what the page shows (`dashboard/test-results/`, a claim's lease renewed) could be left out, or reloads coalesced
   (one per second at most), which also bounds whatever leaks per reload.

**Which process Hakan saw** (CR-140): *Agentic Task Manager* in Activity Monitor. The page's process is listed
apart from it (as the app's *Web Content*, or `com.apple.WebKit.WebContent`), so the next time it is worth checking
both rows: in the hour sampled here the app's own process never passed 163 MB, while its page's reached 975 MB twice.
If the app's own row is the one at gigabytes, the harness of follow-up 1 samples it too, and the cause is on the Rust
side (the subscriptions' watchers, atm-lib's caches) instead.
