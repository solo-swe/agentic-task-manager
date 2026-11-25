# The sidebar overhaul: a scope selector and six destinations (ATM-728)

**Status:** design, agreed with Hakan (CR-135); §7 records the decisions. The build becomes tasks of this sprint
(§8). Inbox and Documents have design documents of their own ([inbox](inbox-design.md),
[documents](documents-design.md), agreed in CR-136).

Builds on [the tabs design](tabs-and-navigation-design.md) (a tab is a place with its own history, every page has an
address) and replaces the sidebar parts of [the desktop UI design](app-desktop-ui-design.md) §4.

## 1. In short

- **Today the app's sidebar only picks the current tab's project**, which the title bar's project chooser (the
  chevron) already does. It lists *Projects Overview* and the workspace's projects, and a click changes this tab
  (`Sidebar.svelte`).
- **It becomes a scope selector and six destinations.** At its top, the **scope**: *All projects* or one project.
  Under it, six items for a project and four for All, each opening that scope's page **in a new tab**:

  | Item | Icon (Lucide) | One project | All projects |
  |---|---|---|---|
  | **Overview** | `layout-dashboard` | the Project tab, with links into the Board (§2.4) | the project cards |
  | **Inbox** (new) | `inbox` | the project's inbox | — |
  | **Tasks** | `square-kanban` | Board · List · Dependencies | Board · List |
  | **Code Reviews** | `file-diff` | the Reviews tab | All reviews, with a project filter (§2.5) |
  | **Documents** (new) | `file-text` | the project's documents | — |
  | **Analysis** | `trending-up` | Progress · Agents | Progress · Agents |

  All projects has no Inbox or Documents for now (Hakan, CR-135; to reconsider later): an inbox and a document are a
  project's.
- **The seven view tabs become six items**; two of them have sub-tabs (Tasks: Board, List, Dependencies; Analysis:
  Progress, Agents). Kanban is renamed **Board** and Graph **Dependencies**, as the items name them.
- **A project opens on its Overview**, no longer on its Board.
- **A tab's title** is `<scope> - <item>`, with the item's icon: `ATM - Tasks`, `All - Code Reviews`.
- **The app loses the title bar's project chooser**; the webapp keeps it, and shows the items as its tabs.

## 2. The sidebar (app)

```
┌──────────────────────────┐
│ ●●●   ‹ ›   ▢            │  the top strip, as now: traffic lights, Back and Forward, the sidebar toggle
├──────────────────────────┤
│ [logo] ATM          ▾  3 │  the scope: logo, name, what needs you; opens the chooser
│ ▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔       │  its progress bar (the active sprint's, as the project card's)
├──────────────────────────┤
│ ▦  Overview              │
│ ✉  Inbox              2  │  new items
│ ▥  Tasks                 │
│ ⎘  Code Reviews       1  │  waiting on you (ATM-733: unseen updates)
│ ▤  Documents             │
│ ↗  Analysis              │
└──────────────────────────┘
```

### 2.1 The scope selector

- **What it lists:** *All projects* first, then the workspace's projects, each as the sidebar lists them today: logo,
  name, progress bar, what needs you as a badge; missing projects dimmed and disabled.
- **The workspace** isn't in it. The workspace switcher stays as it is (`WorkspaceSelector.svelte`, shown only with
  more than one workspace, which only the CLI makes); no new workspace UI (Hakan, CR-135).
- **Choosing a scope changes the sidebar only**, never a tab: the items then open that scope's pages. The current
  tab keeps what it shows.
- **It follows the active tab:** switching to a tab of another project (or of All) selects that scope, so the items
  open next to what you look at. Choosing a scope by hand holds until the next tab switch.
- **Kept across launches** (in the preferences, as the sidebar's width is).
- A Bits UI menu (T-431): arrows, typing, Enter, Escape; *All projects* has the `layout-grid` icon.

### 2.2 The items

- **A click always opens a new tab**, right of the active one, and goes to it, even when another tab already shows
  that page, as a browser's bookmarks do. Never the current tab: the sidebar is where places start, and the tab is
  where you move within one (§3).
- **⌘/Ctrl-click or a middle click** opens it in the background; right-click is the link menu of ATM-699 (Open,
  Open in New Tab, Open in New Window, Copy Link), where *Open* is the plain click's new tab.
- **The current item** is marked when the active tab shows that scope and item (any sub-tab, anything open over it).
- **Badges:** Inbox, its items not triaged yet (`created`); Code Reviews, the reviews waiting on you (what the
  scope's needs-you count holds today for reviews), and ATM-733's unseen updates once built. No badge on the others.
- **Each item is a real link** (`<a href>`, the address of §4), so *Copy Link* and drag work as everywhere else.

### 2.3 What leaves the title bar (app)

- **The project chooser** (`ProjectSelector`, the chevron): its job is the scope selector's now. A tab no longer
  changes project in place; you open the other project's page from the sidebar.
- **The view tabs** (Kanban, List, …) above the page: the sidebar's items replace them. The sub-tabs of Tasks and
  Analysis stay in the page, under its header, as the review page's Overview · Diffs · Comments are.
- **With the sidebar hidden** the new tab's chooser (§3) reaches every page, so a hidden sidebar loses nothing. No
  keyboard shortcuts for the items (Hakan, CR-135).

### 2.4 A project's Overview links into the Board

A project now opens on its Overview (Hakan, CR-135), so the Overview is where the Board is reached from, in place in
the tab:
- **the active sprint** (its name and progress): the Board on that sprint (`/projects/<id>/kanban?sprint=<name>`);
- **each planned and recent closed sprint** it lists, and the backlog: the Board on that one;
- **a count by status** (to do, in review, blocked): the List filtered by it.

What the Overview shows otherwise stays as the Project tab has it today; the links are added where it names a sprint
or counts tasks.

### 2.5 All projects' Code Reviews

All reviews (`/reviews`, ATM-575, today's overview tab) is All's Code Reviews item (Hakan, CR-135), with a **project
filter** at the top of the list: *All projects* or one or more projects, in the list's address like its other filters
(`?projects=atm,compmd`, the tabs design §4.4), beside the search that moves up into the tab's header line (§6).

## 3. Tabs and navigation

- **The + button (and File ▸ New Tab, ⌘T)** stays, and opens a new tab on a **chooser of scope and item** (Hakan,
  CR-135), as a browser's new tab page is. Today it opens the workspace's overview (`tab-actions.ts`).

  ```
  ┌─────────────────────────────────────────────┐
  │  Open                                       │
  │  ┌───────────────┐  ┌─────────────────────┐ │
  │  │ All projects  │  │ ▦ Overview          │ │
  │  │▸ATM        3  │  │ ✉ Inbox          2  │ │
  │  │ compmd        │  │ ▥ Tasks             │ │
  │  │ lfb           │  │ ⎘ Code Reviews   1  │ │
  │  └───────────────┘  │ ▤ Documents         │ │
  │                     │ ↗ Analysis          │ │
  │                     └─────────────────────┘ │
  └─────────────────────────────────────────────┘
  ```

  - **Two columns:** the scopes (All, then the workspace's projects, with their logos and badges, as the scope
    selector lists them) and the items of the scope chosen (six for a project, four for All), with their badges.
    The sidebar's scope is chosen when it opens.
  - **Choosing an item opens it in this tab**, in place of the chooser: the chooser is the new tab's first entry, so
    Back from the page returns to it.
  - **Keys:** ↑↓ move in a column, ←→ between them, typing filters the projects, Enter opens.
  - **Its address** is `/new` (the tab's title: *New Tab*), so a reload, Back and a restored window keep it.
  - **The webapp** doesn't have it: a browser's new tab is the browser's.

  So the sidebar's items are "go straight there" and + is "choose where", and with the sidebar hidden the chooser
  is the way to every page.
- **A tab's title** is `<scope> - <item>` with the item's icon: `ATM - Tasks`, `All - Analysis`. *Scope* is the
  project's **key** (`ATM`) when it has one, else its name cut to 16 characters; All is `All`. A review or a
  document open in the tab names itself instead: `ATM - CR-134`, `ATM - sidebar-overhaul-design.md` (the item's icon
  kept).
- **A link inside a tab opens in the same tab**, as the tabs design has it: a review from the board, a task from the
  list, a document from a task, the Board from the Overview. Back walks the tab's own history, so it returns to where
  you were. Nothing in here changes the tabs design; the sidebar's new tab is the one exception to "a click opens in
  place", and it is one because the sidebar isn't in any tab.
- **A link to another scope** (a task's dependency in another project, a project card in All's Overview, a review in
  All's Code Reviews) also opens in place: the tab's scope is whatever its page is, and its title follows. The tab is a
  history, not a project.
- **The tab's sub-tabs** (Board, List, Dependencies; Progress, Agents) are steps in its history, as the view tabs are
  now.

## 4. Addresses

The items are groups of today's views, so the addresses keep their view segment, with two new views and the All
scope's list:

| Item | One project | All projects |
|---|---|---|
| Overview | `/projects/<id>/project`, and `/projects/<id>` | `/projects` |
| Inbox | `/projects/<id>/inbox` (new) | — |
| Tasks | `/projects/<id>/kanban`, `/list`, `/graph` | `/board`, `/list` (new) |
| Code Reviews | `/projects/<id>/reviews`, `…/reviews/<CR>` | `/reviews`, `?projects=` (new) |
| Documents | `/projects/<id>/documents` (new); a document open as `?doc=` | — |
| Analysis | `/projects/<id>/progress`, `/agents` | `/progress`, `/agents` |

- **The new tab's chooser** is `/new` (the app's only).
- **`/projects/<id>` with no view is the Overview** now, no longer the Board. An old link to it lands on the
  Overview, a click from the Board; every other address is unchanged.
- **Old addresses stay valid**: none is renamed (`kanban` and `graph` keep their segments; only the labels change), so
  deep links, bookmarks and `windows.json`'s saved tabs open as before.
- **The item of an address** is a function of its view (`lib/address.ts`, next to `placeName`), the one rule both
  the sidebar's mark and the tab's title use.
- `docs/multi-project-api.md` §2.7 and `PAGE_PATH` get the new segments.

## 5. The webapp

- **No sidebar**, as today: one page, the title bar with the project chooser and the workspace switcher.
- **The items are its tabs**, in the sidebar's order, under the title bar, where the seven view tabs are now; a tab
  click is in place (the browser's tab is the place). Tasks and Analysis show their sub-tabs under it.
- **For All projects** its four: today's overview tabs Projects, Omniboard, Progress, Agents and All reviews map onto
  Overview, Tasks, Analysis and Code Reviews.
- So the app and the webapp show the same page for an address: the app takes the item row away, the sidebar does its
  job.

## 6. Moved inside the tab (ATM-736, ATM-737, ATM-738)

- **The All-projects activity pill** (`OverviewActivity.svelte`: in the app, at the window title bar's right end
  beside the workspace switcher, `Header.svelte` `appEnd`) moves into the tab's header line, at its right end, where a
  project's Progress and Activity pills are (`tabPills`). It belongs to the All scope's pages, not to the window.
- **The All-projects search** and **the Reviews search** (a project's and All's) move up into the tab's header line,
  at its left, where the project search and Filters are (`tabSearch`): the same field, the same place, on every page
  that searches. The page's own row they sit in now goes; All reviews' project filter (§2.5) sits with its search.

## 7. Decisions (Hakan, CR-135)

1. **The scope follows the active tab**; a hand choice holds until the next switch.
2. **No new workspace UI**: the switcher stays as it is.
3. **An item always opens a new tab**, even when one already shows it.
4. **A project opens on its Overview**, which links into the Board (§2.4).
5. **A tab's scope is the project key** (`ATM - Tasks`).
6. **All projects has Code Reviews**, with a project filter (§2.5); no Inbox or Documents for now.
7. **+ opens a chooser** of scope and item (§3).
8. **No keyboard shortcuts** for the items, for now.

## 8. Tasks it implies

1. **The item model:** an address's item, the new views `inbox`, `documents` and All's `list` in `lib/address.ts` and
   `PAGE_PATH`; `/projects/<id>` opening the Overview; labels Board and Dependencies; `multi-project-api.md` §2.7.
2. **The page by item:** the items as the webapp's tabs, with Tasks' and Analysis' sub-tabs; the All scope's List.
3. **The sidebar:** the scope selector and the items, badges, new tab on click, the link menu; the title bar's
   project chooser and view tabs gone in the app.
4. **Tab titles:** `<scope> - <item>` and its icon, from the item rule.
5. **The new tab's chooser:** `/new`, its two columns and keys, + and ⌘T opening it.
6. **A project's Overview links into the Board** (§2.4).
7. **All reviews' project filter** (§2.5), in its address.
8. **The pill and the searches inside the tab:** ATM-736, ATM-737, ATM-738 (exist).
9. **Inbox:** from `docs/inbox-design.md` (its own tasks: data model and atm commands, the route, the page, agents'
   triage).
10. **Documents:** from `docs/documents-design.md` (the listing route, the page).
11. **Docs:** `app-desktop-ui-design.md` §4 replaced by this; Help's pages on the sidebar and tabs.
