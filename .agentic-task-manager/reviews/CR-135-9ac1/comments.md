## docs/sidebar-overhaul-design.md:82 · round 1 · t1
> ## 3. Tabs and navigation

- [x] **Hakan Serce** · 2026-11-23T22:51:21Z · c1
  What happens to the + (add new tab) button?
  - **claude:ba530ebb** · 2026-11-23T22:51:51Z · c2
    It stays, along with File ▸ New Tab and ⌘T. It opens a new tab on the Overview of the scope selected in the sidebar (that project’s Overview, or the project cards for All). Today it always opens the workspace overview. The six items go straight to a page; + is a fresh start in the current scope, like a browser’s new tab page. Added at the top of §3. Should it do something else, like duplicate the current tab?
    - **Hakan Serce** · 2026-11-23T22:53:05Z · c3
      I can show a selector of scope and items?
      - **claude:ba530ebb** · 2026-11-23T22:53:50Z · c4
        Yes. + (and ⌘T) now opens a New Tab page at `/new`. It has two columns: the scopes (All, then the projects, with badges) and the six items of the chosen scope. The sidebar’s scope is preselected. Choosing an item opens it in place of the chooser, so Back returns to the chooser. It works from the keyboard: arrows, typing filters the projects, Enter opens, ⌘1–⌘6 open an item at once. It is in §3 with a sketch, plus §2.3, §4 and §8; question 8 is now only about ⌘1–⌘6 outside the chooser. The webapp keeps the browser’s own new tab.

## docs/sidebar-overhaul-design.md:153 · round 3 · t2
> - **For All projects** the same six (today's overview tabs Projects, Omniboard, Progress, Agents, All reviews map onto

- [x] **Hakan Serce** · 2026-11-23T23:02:11Z · c5
  We don't need Code Reviews, Inbox, and Documents tabs for All projects. Maybe later we can reconsider.
  - **claude:ba530ebb** · 2026-11-23T23:03:00Z · c6
    Done. All projects now has three items: Overview, Tasks (Board · List) and Analysis. The sidebar, the New Tab chooser and the webapp’s tabs list only those. I updated §1, §4 and §5. The old question 7 is settled. Question 6 now asks about today’s All reviews page (`/reviews`): I suggest keeping it as an address nothing links to, so old links and saved tabs still open, rather than removing it.

## docs/sidebar-overhaul-design.md:174 · round 4 · t3
> 1. **The scope and the active tab:** the scope follows the active tab, and a hand choice holds until the next switch

- [x] **Hakan Serce** · 2026-11-23T23:12:54Z · c7
  let's go with the recommendation
  - **claude:ba530ebb** · 2026-11-23T23:14:08Z · c16
    Done: the scope follows the active tab, and a choice made by hand holds until the next tab switch (§2.1, §7.1).

## docs/sidebar-overhaul-design.md:176 · round 4 · t4
> 2. **The workspace picker:** stays in the title bar (recommended: a workspace is rarely changed), or moves into the

- [x] **Hakan Serce** · 2026-11-23T23:12:54Z · c8
  Do we have a workspace picker today? We have the infra but no UI, correct? Let's keep it that way for now.
  - **claude:ba530ebb** · 2026-11-23T23:14:08Z · c17
    Close. There is a UI, `WorkspaceSelector.svelte`, but it shows only when there is more than one workspace, and only the CLI (`atm workspace create`) can make a second one. So in practice nobody sees it. It stays exactly as it is, with no new workspace UI (§2.1, §7.2).

## docs/sidebar-overhaul-design.md:178 · round 4 · t5
> 3. **An item already open in a tab:** always a new tab, as asked (recommended, as a browser's bookmarks)? Or go to

- [x] **Hakan Serce** · 2026-11-23T23:12:54Z · c9
  recommended
  - **claude:ba530ebb** · 2026-11-23T23:14:09Z · c18
    Done: a sidebar item always opens a new tab, even when another tab already shows that page (§2.2, §7.3).

## docs/sidebar-overhaul-design.md:180 · round 4 · t6
> 4. **A project's default page** (`/projects/<id>`, a scope's first click): the Board, as today, or its Overview?

- [x] **Hakan Serce** · 2026-11-23T23:12:54Z · c10
  Overview, and overview can have links to sprints, etc that open the Board.
  - **claude:ba530ebb** · 2026-11-23T23:14:10Z · c19
    Done. A project now opens on its Overview (`/projects/<id>` is the Overview, not the Board), and the Overview links into the Board in the same tab: the active sprint, each planned or recently closed sprint and the backlog open the Board on that sprint, and the counts by status open the filtered List. See the new §2.4 and §4.

## docs/sidebar-overhaul-design.md:181-182 · round 4 · t7
> 5. **The tab's scope name:** the project key (`ATM - Tasks`, recommended: short and what tasks are called by), or
>    the project's name?

- [x] **Hakan Serce** · 2026-11-23T23:12:54Z · c11
  yes
  - **claude:ba530ebb** · 2026-11-23T23:14:10Z · c20
    Done: tab titles use the project key, as in `ATM - Tasks` (§3, §7.5).

## docs/sidebar-overhaul-design.md:139 · round 4 · t8
> | Code Reviews | `/projects/<id>/reviews`, `…/reviews/<CR>` | — (`/reviews` today) |

- [x] **Hakan Serce** · 2026-11-23T23:12:54Z · c12
  actually, we already have the capability today, let's have reviews in all projects as well. Then we will need project filter on top of these reviews.
  - **claude:ba530ebb** · 2026-11-23T23:14:11Z · c21
    Done. All projects has Code Reviews back: today’s All reviews page (`/reviews`), with a project filter at the top of the list (All projects, or one or more), kept in the address as `?projects=` like the list’s other filters. That makes four items for All. See the new §2.5, plus §1, §4 and §5.

## docs/sidebar-overhaul-design.md:183-184 · round 4 · t9
> 6. **All reviews (`/reviews`)**, now with no item: kept as an address nothing links to (recommended: old links and
>    saved tabs keep working, and it is there when All gets Code Reviews back), or removed, its address opening All's

- [x] **Hakan Serce** · 2026-11-23T23:12:54Z · c13
  see my other comment, I think we will bering All Projects reviews back, so this will be functional.
  - **claude:ba530ebb** · 2026-11-23T23:14:12Z · c22
    Agreed. `/reviews` is All’s Code Reviews again, so the question is gone (§7.6).

## docs/sidebar-overhaul-design.md:186 · round 4 · t10
> 7. **Settled (CR-135):** All projects has no Inbox, Code Reviews or Documents for now; an inbox is a project's.

- [x] **Hakan Serce** · 2026-11-23T23:12:54Z · c14
  see my other comments, leave Code Reviews (we will have it for All )
  - **claude:ba530ebb** · 2026-11-23T23:14:12Z · c23
    Done. All projects has no Inbox or Documents for now, and does have Code Reviews (§1, §7.6).

## docs/sidebar-overhaul-design.md:187-188 · round 4 · t11
> 8. **⌘1 … ⌘6:** the six items, in the new tab's chooser only (recommended: ⌘1 … ⌘9 select tabs in browsers, and
>    atm's tabs may want them later)? Or also everywhere, opening a new tab on the sidebar's scope?

- [x] **Hakan Serce** · 2026-11-23T23:12:54Z · c15
  No need for these shortcuts, for now.
  - **claude:ba530ebb** · 2026-11-23T23:14:13Z · c24
    Done. There are no keyboard shortcuts for the items, ⌘1–⌘6 are gone from the chooser, and the Go menu items are dropped. With the sidebar hidden, the New Tab chooser reaches every page (§2.3, §3, §7.8).
