+++
id = "01a20e0c-516c-71b6-aee2-410de92f7525"
number = 156
title = "Reviews search moves up in the tab"
status = "approved"
tasks = ["ATM-738"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T14:53:38Z
updated = 2026-11-24T16:21:04Z
+++

The reviews search moves up to where a project's search is, on a project's Code Reviews and on All's (ATM-738, [the design §6](docs/sidebar-overhaul-design.md#6-moved-inside-the-tab-atm-736-atm-737-atm-738)). In the app it sits at the left of the tab's line, in the browser among the title bar's charts, before the pills.

**The change:**
- `reviews/ReviewSearch.svelte`: the field, with the list's debounce (a search a moment after typing stops, trimmed) and a project search's look. It folds to a magnifier until used, has × to clear, and `/` focuses it. The placeholder is now "Search reviews"; the tooltip says what it searches.
- `reviews/search.svelte.ts`: the list's query is its page's (a project's in the address, All's kept in the browser), so `ReviewList` *offers* its search while it is shown: the query's text, and a way to search. `Header.svelte` shows the field while one is offered. Taking an offer back clears only that offer.
- `ReviewList.svelte`: its own search box is gone. The bar under it shows only when it has *Next review needing you* or *New live review*.

**One choice to check:** on a project's Code Reviews tab, the reviews search *takes the place of* the tasks' search and Filters. They filter tasks, which that tab doesn't show, and the design asks for one search where a page's search is. They come back on the other tabs. Say if you'd rather keep the tasks' Filters there too.

All reviews' project picker stays where it is for now. ATM-757 (its project filter, "sits with its search") moves it.

**Tests:**
- `ReviewSearch.component.test.ts`: shows the query's text, searches once typing stops (trimmed), × clears at once and drops a pending search, `/` focuses it (not while typing elsewhere, not with ⌘ or Ctrl). The offer registry: the last offer wins, and taking one back leaves the other's.
- `ReviewList.component.test.ts`: offers its search while shown, reads a new query, takes the offer back when unmounted. There is no search box in the list.
- `reviews.spec.ts`: the search is in the title bar before the pills, with no tasks search or Filters there. `/` focuses it, Kanban brings the tasks' search back, and All's Code Reviews has it in the title bar. The existing search, filter and address tests pass unchanged.
- `app.spec.ts`: a project's Code Reviews has the reviews search in the tab's line, not the tasks'. All's Code Reviews has it at the left of the line, before the subtitle.
- `pnpm run check`, the unit suite (2017 passed), the functional e2e suite (614 passed).
