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
  Under it, six items for a project and three for All, each opening that scope's page **in a new tab**:

  | Item | Icon (Lucide) | One project | All projects |
  |---|---|---|---|
  | **Overview** | `layout-dashboard` | the Project tab | the project cards |
  | **Inbox** (new) | `inbox` | the project's inbox | — |
  | **Tasks** | `square-kanban` | Board · List · Dependencies | Board · List |
  | **Code Reviews** | `file-diff` | the Reviews tab | — |
  | **Documents** (new) | `file-text` | the project's documents | — |
  | **Analysis** | `trending-up` | Progress · Agents | Progress · Agents |

  All projects has no Inbox, Code Reviews or Documents for now (Hakan, CR-135; to reconsider later): the sidebar
  lists only its three items.

- **The seven view tabs become six items**; two of them have sub-tabs (Tasks: Board, List, Dependencies; Analysis:
  Progress, Agents). Kanban is renamed **Board** and Graph **Dependencies**, as the items name them.
- **A tab's title** is `<scope> - <item>`, with the item's icon: `ATM - Tasks`, `All - Analysis`.
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
- **With the sidebar hidden** the new tab's chooser (§3) reaches every page, and the View menu gets the six as
  **Go ▸ Overview … Analysis** (opening a new tab on the sidebar's scope), so a hidden sidebar loses nothing.

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
    selector lists them) and the items of the scope chosen (six for a project, three for All), with their badges. The sidebar's scope is chosen
    when it opens.
  - **Choosing an item opens it in this tab**, in place of the chooser: the chooser is the new tab's first entry, so
    Back from the page returns to it.
  - **Keys:** ↑↓ move in a column, ←→ between them, typing filters the projects, Enter opens; ⌘1 … ⌘6 open that item
    of the chosen scope at once.
  - **Its address** is `/new` (the tab's title: *New Tab*), so a reload, Back and a restored window keep it.
  - **The webapp** doesn't have it: a browser's new tab is the browser's.

  So the sidebar's items are "go straight there" and + is "choose where", and with the sidebar hidden the chooser
  is the way to every page.
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
| Inbox | `/projects/<id>/inbox` (new) | — |
| Tasks | `/projects/<id>/kanban`, `/list`, `/graph` | `/board`, `/list` (new) |
| Code Reviews | `/projects/<id>/reviews`, `…/reviews/<CR>` | — (`/reviews` today) |
| Documents | `/projects/<id>/documents` (new); a document open as `?doc=` | — |
| Analysis | `/projects/<id>/progress`, `/agents` | `/progress`, `/agents` |

- **The new tab's chooser** is `/new` (the app's only).
- **All reviews (`/reviews`) leaves the items**: nothing links to it any more, but the address still opens the list, so
  old links and saved tabs work (§7, question 6).
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
- **For All projects** its three (today's overview tabs Projects, Omniboard, Progress and Agents map onto Overview,
  Tasks and Analysis; All reviews has no tab).
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
6. **All reviews (`/reviews`)**, now with no item: kept as an address nothing links to (recommended: old links and
   saved tabs keep working, and it is there when All gets Code Reviews back), or removed, its address opening All's
   Overview?
7. **Settled (CR-135):** All projects has no Inbox, Code Reviews or Documents for now; an inbox is a project's.
8. **⌘1 … ⌘6:** the six items, in the new tab's chooser only (recommended: ⌘1 … ⌘9 select tabs in browsers, and
   atm's tabs may want them later)? Or also everywhere, opening a new tab on the sidebar's scope?

## 8. Tasks it implies

1. **The item model:** an address's item, the new views `inbox`, `documents` and All's `list` in `lib/address.ts` and
   `PAGE_PATH`; labels Board and Dependencies; `multi-project-api.md` §2.7.
2. **The page by item:** the six items as the webapp's tabs, with Tasks' and Analysis' sub-tabs; the All scope's List.
3. **The sidebar:** the scope selector and the six items, badges, new tab on click, the link menu; the title bar's
   project chooser and view tabs gone in the app; Go menu items.
4. **Tab titles:** `<scope> - <item>` and its icon, from the item rule.
5. **The new tab's chooser:** `/new`, its two columns and keys, + and ⌘T opening it.
6. **The pill and the searches inside the tab:** ATM-736, ATM-737, ATM-738 (exist).
7. **Inbox:** from `docs/inbox-design.md` (its own tasks: data model and atm commands, the route, the page, agents'
   triage).
8. **Documents:** from `docs/documents-design.md` (the listing route, the page).
9. **Docs:** `app-desktop-ui-design.md` §4 replaced by this; Help's pages on the sidebar and tabs.
