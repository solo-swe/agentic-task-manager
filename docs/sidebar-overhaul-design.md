# The sidebar overhaul: a scope selector and six destinations (ATM-728)

**Status:** design, for review with Hakan (CR). Nothing is built until it is agreed; the build becomes tasks of this
sprint (§8). Inbox and Documents get design documents of their own (`docs/inbox-design.md`,
`docs/documents-design.md`), written once this one is agreed.

Builds on [the tabs design](tabs-and-navigation-design.md) (a tab is a place with its own history, every page has an
address) and replaces the sidebar parts of [the desktop UI design](app-desktop-ui-design.md) §4.

## 1. In short

- **Today the app's sidebar only picks the current tab's project**, which the title bar's project chooser (the
  chevron) already does. It lists *Projects Overview* and the workspace's projects, and a click changes this tab
  (`Sidebar.svelte`).
- **It becomes a scope selector and six destinations.** At its top, the **scope**: *All projects* or one project.
  Under it, six items, each opening that scope's page **in a new tab**:

  | Item | Icon (Lucide) | One project | All projects |
  |---|---|---|---|
  | **Overview** | `layout-dashboard` | the Project tab | the project cards |
  | **Inbox** (new) | `inbox` | the project's inbox | every project's inbox, by project |
  | **Tasks** | `square-kanban` | Board · List · Dependencies | Board · List |
  | **Code Reviews** | `file-diff` | the Reviews tab | All reviews |
  | **Documents** (new) | `file-text` | the project's documents | every project's, by project |
  | **Analysis** | `trending-up` | Progress · Agents | Progress · Agents |

- **The seven view tabs become six items**; two of them have sub-tabs (Tasks: Board, List, Dependencies; Analysis:
  Progress, Agents). Kanban is renamed **Board** and Graph **Dependencies**, as the items name them.
- **A tab's title** is `<scope> - <item>`, with the item's icon: `ATM - Tasks`, `All - Code Reviews`.
- **The app loses the title bar's project chooser**; the webapp keeps it, and shows the six items as its tabs.

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
  name, progress bar, what needs you as a badge; missing projects dimmed and disabled. The workspace picker stays
  in the title bar (§7, question 2).
- **Choosing a scope changes the sidebar only**, never a tab: the items then open that scope's pages. The current
  tab keeps what it shows.
- **It follows the active tab:** switching to a tab of another project (or of All) selects that scope, so the items
  open next to what you look at. Choosing a scope by hand holds until the next tab switch.
- **Kept across launches** (in the preferences, as the sidebar's width is).
- A Bits UI menu (T-431): arrows, typing, Enter, Escape; *All projects* has the `layout-grid` icon.

### 2.2 The items

- **A click opens a new tab**, right of the active one, and goes to it. Never the current tab: the sidebar is where
  places start, and the tab is where you move within one (§3).
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
- **With the sidebar hidden** the items aren't anywhere: the View menu gets the six as **Go ▸ Overview … Analysis**
  (⌘1 … ⌘6, opening a new tab), so a hidden sidebar loses nothing.

## 3. Tabs and navigation

- **The + button (and File ▸ New Tab, ⌘T)** stays, and opens a new tab on the **sidebar's scope's Overview**: the
  selected project's Overview, or All's project cards. Today it opens the workspace's overview whatever is selected
  (`tab-actions.ts`). So + is "a fresh start here", as a browser's new tab page is, and the six items are "go
  straight there"; with the sidebar hidden, + is still the way to a new tab.
- **A tab's title** is `<scope> - <item>` with the item's icon: `ATM - Tasks`, `All - Analysis`. *Scope* is the
  project's **key** (`ATM`) when it has one, else its name cut to 16 characters; All is `All`. A review or a
  document open in the tab names itself instead: `ATM - CR-134`, `ATM - sidebar-overhaul-design.md` (the item's icon
  kept).
- **A link inside a tab opens in the same tab**, as the tabs design has it: a review from the board, a task from the
  list, a document from a task. Back walks the tab's own history, so it returns to where you were. Nothing in here
  changes the tabs design; the sidebar's new tab is the one exception to "a click opens in place", and it is one
  because the sidebar isn't in any tab.
- **A link to another scope** (a task's dependency in another project, a project card in All's Overview) also opens in
  place: the tab's scope is whatever its page is, and its title follows. The tab is a history, not a project.
- **The tab's sub-tabs** (Board, List, Dependencies; Progress, Agents) are steps in its history, as the view tabs are
  now.

## 4. Addresses

The items are groups of today's views, so the addresses keep their view segment, with two new views and the All
scope's list:

| Item | One project | All projects |
|---|---|---|
| Overview | `/projects/<id>/project` | `/projects` |
| Inbox | `/projects/<id>/inbox` (new) | `/inbox` (new) |
| Tasks | `/projects/<id>/kanban`, `/list`, `/graph` | `/board`, `/list` (new) |
| Code Reviews | `/projects/<id>/reviews`, `…/reviews/<CR>` | `/reviews` |
| Documents | `/projects/<id>/documents` (new); a document open as `?doc=` | `/documents` (new) |
| Analysis | `/projects/<id>/progress`, `/agents` | `/progress`, `/agents` |

- **Old addresses stay valid**: none is renamed (`kanban` and `graph` keep their segments; only the labels change), so
  deep links, bookmarks and `windows.json`'s saved tabs open as before.
- **The item of an address** is a function of its view (`lib/address.ts`, next to `placeName`), the one rule both
  the sidebar's mark and the tab's title use.
- `/projects/<id>` with no view stays the Board, as today's default (§7, question 4).
- `docs/multi-project-api.md` §2.7 and `PAGE_PATH` get the new segments.

## 5. The webapp

- **No sidebar**, as today: one page, the title bar with the project chooser and the workspace picker.
- **The six items are its tabs**, in the sidebar's order, under the title bar, where the seven view tabs are now; a
  tab click is in place (the browser's tab is the place). Tasks and Analysis show their sub-tabs under it.
- **For All projects** the same six (today's overview tabs Projects, Omniboard, Progress, Agents, All reviews map onto
  Overview, Tasks, Analysis, Code Reviews; Inbox and Documents are new).
- So the app and the webapp show the same page for an address: the app takes the item row away, the sidebar does its
  job.

## 6. Moved inside the tab (ATM-736, ATM-737, ATM-738)

- **The All-projects activity pill** (`OverviewActivity.svelte`: in the app, at the window title bar's right end
  beside the workspace picker, `Header.svelte` `appEnd`) moves into the tab's header line, at its right end, where a
  project's Progress and Activity pills are (`tabPills`). It belongs to the All scope's pages, not to the window.
- **The All-projects search** and **the Reviews search** move up into the tab's header line, at its left, where the
  project search and Filters are (`tabSearch`): the same field, the same place, on every page that searches. The
  page's own row they sit in now goes.

## 7. Questions for Hakan

1. **The scope and the active tab:** the scope follows the active tab, and a hand choice holds until the next switch
   (recommended)? Or only by hand, so the sidebar stays where you left it?
2. **The workspace picker:** stays in the title bar (recommended: a workspace is rarely changed), or moves into the
   scope selector, above its projects?
3. **An item already open in a tab:** always a new tab, as asked (recommended, as a browser's bookmarks)? Or go to
   the tab already showing that scope's item, and ⌘-click for a new one?
4. **A project's default page** (`/projects/<id>`, a scope's first click): the Board, as today, or its Overview?
5. **The tab's scope name:** the project key (`ATM - Tasks`, recommended: short and what tasks are called by), or
   the project's name?
6. **All projects' Inbox:** one inbox for each project only, shown together for All (recommended)? Or also a
   user-level inbox for input not tied to a project yet, triaged into one later? (For `docs/inbox-design.md`.)
7. **Documents for All:** every project's documents, by project (recommended), or Documents for one project only,
   like Dependencies?
8. **The sidebar hidden:** Go ▸ Overview … Analysis with ⌘1 … ⌘6 (recommended)? ⌘1 … ⌘9 select tabs in browsers;
   atm's tabs today have Ctrl+Tab only.

## 8. Tasks it implies

1. **The item model:** an address's item, the new views `inbox`, `documents` and All's `list` in `lib/address.ts` and
   `PAGE_PATH`; labels Board and Dependencies; `multi-project-api.md` §2.7.
2. **The page by item:** the six items as the webapp's tabs, with Tasks' and Analysis' sub-tabs; the All scope's List.
3. **The sidebar:** the scope selector and the six items, badges, new tab on click, the link menu; the title bar's
   project chooser and view tabs gone in the app; Go menu items.
4. **Tab titles:** `<scope> - <item>` and its icon, from the item rule.
5. **The pill and the searches inside the tab:** ATM-736, ATM-737, ATM-738 (exist).
6. **Inbox:** from `docs/inbox-design.md` (its own tasks: data model and atm commands, the route, the page, agents'
   triage).
7. **Documents:** from `docs/documents-design.md` (the listing route, the page).
8. **Docs:** `app-desktop-ui-design.md` §4 replaced by this; Help's pages on the sidebar and tabs.
