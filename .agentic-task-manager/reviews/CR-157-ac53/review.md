+++
id = "01a20e1a-0e8b-73eb-a3cb-514462d5ac53"
number = 157
title = "Activity pill: the same chart every time"
status = "approved"
tasks = ["ATM-739"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T15:08:38Z
updated = 2026-11-24T16:24:19Z
+++

The Activity pill now shows the same chart every time (ATM-739). Per your decision, it charts the task files' completions and leaves the archive out. The panel's deep dive still counts the archive (D-5.2).

**The cause, on a project:** the pill charted `dashboard.history`, which holds the archive once any view loads it. Opening the panel loads it, and so does the project's Overview. A project loaded again starts without it. So the bars changed on the first click, and again after another page and back. Confirmed in e2e: the pill already counted 8 on opening the project, because its Overview had loaded the archive.

**On All projects: not reproduced.** On the current code, in the browser and in the app (the pill now inside the tab), the bars stayed the same after a click, after Board and back, after a project and Back, and after another app tab and back. Its sources always held each project's archive, so the data didn't differ between loads. It now charts the task files too, so both pills follow one rule. If you still see it change, tell me what you did and I'll chase it. Recorded in the task's notes.

**The change:**
- `ActivityWidget.svelte`: the pill's series comes from `dashboard.tasks`, or on All from the sources' new `liveByProject`. The deep dive's all-time series is now its own and still uses `history` (or All's `byProject`, archives included).
- With an archive, the pill's tooltip and label say "archived tasks left out" (`sparklineTitle(…, archiveLeftOut)`). Without one, they are as before.
- The ⓘ: one line on what the pill charts. All's line said "their archives left out", which hasn't been true since T-355; it now says the archives are included.
- `OverviewActivity.svelte`: `liveByProject` from each source's `live`, and `archived` when any project has one.

**One effect:** a project whose completions are all archived (none in `tasks.json`) shows no pill, as a project with no completions doesn't.

**Tests:**
- `ActivityWidget.component.test.ts`: a project's pill is the same with the archive loaded or not (`history` 4 tasks, pill 2) and says the archive is left out. Without an archive it says nothing of one. The overview's pill is the same whether its charts hold archives or not.
- `insights.test.ts`: the tooltip with the archive left out.
- `widgets.spec.ts`: on a project with an archive, the pill's bars stay the same after opening the panel (the deep dive shows 8 done, the pill 6), after Escape, and after the overview and Back in the page.
- `overview-activity.spec.ts`: T-355's test now has the pill counting 12 and the panel 14, and the bars stay the same after a click and after Board and back.
- `pnpm run check`, the unit suite (2020 passed), the functional e2e suite (615 passed).
