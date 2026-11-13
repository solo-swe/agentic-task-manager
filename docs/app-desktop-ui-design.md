# The atm app as a desktop app

Status: **accepted** (T-510, reviewed with Hakan 2026-11-09; the decisions in §10). §9's steps are tasks.

The atm app (T-455 onwards, the [Tauri guide](guides/tauri.md)) shows the dashboard's page as a browser shows it: a
header with the title, the selectors and the ⋯ menu, and a footer of links, in a plain window. This document says what
changes to make it a desktop app, and what stays the same as the browser dashboard.

What this aims for:

- **One desktop app, the same on macOS, Windows and Linux** (Hakan): the same window, tabs, sidebar, status bar, menus
  and shortcuts on all three, from one implementation. The browser dashboard is the other face, and keeps working as
  it does; a mobile app, if one comes, has its own (§1a).
- **One Svelte code base:** the app's shell is a variant of the page's shell, switched on `inApp`
  (`lib/api/http.ts`); the views, dialogs, state and addresses are shared. The browser's e2e suite and screenshots
  don't change.
- **Native where it matters to the platform:** macOS's menu bar at the top of the screen and its window buttons;
  each platform's window behaviour (dragging, zoom, snapping). Where a platform has a feature the others don't
  (macOS's window tabs), the app doesn't use it: the app's own works the same everywhere.
- **Only Tauri's documented, stable APIs,** and no new plugin without a yes (the [Rust guide](guides/rust.md)).
- **The page's privileges stay as narrow:** it calls only the app's own commands (I25), never Tauri's window or menu
  API directly.

## 1. What the window has now

| Part | Now (`dashboard/src/lib/shell/`) |
|---|---|
| Title | `TitleBar.svelte`: logo, title, `ProjectSelector`, `WorkspaceSelector`, version chip, subtitle (counts, needs-you), charts and filters, `RefreshMenu`, `SettingsMenu` (⋯) |
| Views | `ViewPanel.svelte`: Graph, Kanban, List, Agents, Progress, Project as tabs in the page |
| Footer | `Footer.svelte`: brand, copyright, archive and problems notices, version (What's new), GitHub, Connect your agents (app only), Help, About |
| Window | one window, `main` (`tauri.conf.json`), standard macOS title bar with the title "atm"; workspaces switch in place (T-457) |
| App | `lib.rs`: one instance (the second focuses `main`), deep links to `main` (T-459), subscriptions for the page's changes (T-506), all subscriptions ended when the page loads again (`unwatch_all`) |

## 1a. Platforms

The app ships on macOS now; Windows and Linux come next (their tests in T-482, the deep-link plugin when the app ships
there). iOS and Android aren't planned.

**The window is the page's on all three desktops.** The page draws the title bar, the tabs, the sidebar, the status
bar and, where the platform has no menu bar of its own, the menus. What stays the platform's:

| Part | macOS | Windows | Linux |
|---|---|---|---|
| Window buttons (§2) | the system's traffic lights, over the page's bar (overlay title bar) | the page's minimize, maximize and close, at the right, in Windows' look | the page's, at the right, in the desktop's usual look |
| Menus (§6) | the system menu bar, at the top of the screen | the same menus, drawn by the page in its title bar (as VS Code does) | the same as Windows |
| Shortcuts (§6) | ⌘ | Ctrl | Ctrl |
| Badge (§7) | the Dock's | an overlay icon on the taskbar button | the launcher's, where the desktop shows one |

Everything else (tabs, sidebar, status bar, settings, windows coming back) is the same code with the same behaviour.

From Tauri's documentation: the overlay title bar, the hidden title and `trafficLightPosition` are macOS only; menus
"appear as part of the application window for Windows or Linux, or in the menu bar on MacOS", and the menu API is
"available on desktop only"; `hide_menu` works on Windows and Linux ("macOS: Unsupported").

**The page asks about features, not about the OS.** One command, `shell`, says what the window has:
`{ system_menu_bar, system_window_buttons }` (from `cfg!(target_os)`): with a system menu bar the page draws none;
with system window buttons it leaves room for them, otherwise it draws its own. In the browser, none of this.

**Shortcuts are written once,** as `CmdOrCtrl+…` (Tauri's menus map `CmdOrCtrl` to ⌘ on macOS and Ctrl on Windows and
Linux). Where the platforms' conventions differ, each gets its own:

| Action | macOS | Windows, Linux |
|---|---|---|
| Back, Forward | ⌘[, ⌘] | Alt+←, Alt+→ |
| Next, previous tab | ⌃Tab, ⌃⇧Tab (and ⌘⇧], ⌘⇧[) | Ctrl+Tab, Ctrl+Shift+Tab |
| Show Sidebar | ⌃⌘S | Ctrl+B |
| Full Screen | ⌃⌘F | F11 |
| Help | ⌘? | F1 |
| Quit | ⌘Q | Ctrl+Q on Linux; Alt+F4 (the system's) on Windows |
| Open in a new tab | ⌘-click | Ctrl-click; a middle click on all three |

The rest (⌘T, ⌘N, ⌘W, ⌘,, ⌘F, ⌘1–⌘6, ⌘R) are the same keys with Ctrl. The tables below give the macOS keys.

**Mobile, if it comes,** has its own shell: Tauri's menu API and several windows are desktop only, and a phone wants
the page's own navigation. Nothing here stands in its way: every desktop part is behind `shell`, and without it the
page keeps the browser's header and footer.

## 2. The title bar

**One row, drawn by the page, the same on all three** (Hakan, on T-513's first try: a second row under the title bar
looked wrong): the window buttons (macOS: the system's, at the left; Windows and Linux: the page's, at the right),
the sidebar button, **the tabs** (§3) and a **+** for a new one; on Windows and Linux, the menus (§6) at its left.
A tab's label is its project's logo and name (or "Projects Overview"), nothing else.

**On the tab's line, right-aligned** (Hakan, T-513): the workspace picker, the Progress and Activity pills and the ⋯,
which opens the Settings sheet (§6). Search and Filters are inside the tab, before the subtitle, the Filters button
its icon alone (Hakan, T-516). The tab's label has the project picker after the name. The
version chip goes (Help ▸ What's New). **Inside the tab** only the subtitle line (counts, archived, the needs-you
chip), directly on top of the views (Graph, Kanban, List, …), with no hairline. The overview's tab is labelled with
atm's logo. **No reload button:** the page reflects changes as they happen (T-506), and View ▸ Reload (⌘R) stays in the
menu for the rare change the watcher missed.

The window on macOS (the menu bar is the system's, at the top of the screen):

```text
┌────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ o o o  [=]  │ ▣ atm (2)   × │ ▣ compmd    × │ Projects Overview × │ +                              │  title bar: tabs
├────────────────────────┬───────────────────────────────────────────────────────────────────────────┤
│ Search…           ⌘F   │ Graph  Kanban  List │ Agents  Progress  Project       (2)  Filters  ⋯     │  the tab's header
│ ▾ Personal             ├───────────────────────────────────────────────────────────────────────────┤
│   Projects Overview    │ Sprint: The atm app, continued ▾      3 of 6 done  ▓▓▓▓▓▓░░░░             │  scope bar (as now)
│ ▸ ▣ atm           (2)  │                                                                           │
│     ▓▓▓▓▓░░░░░         │                                                                           │
│   ▣ compmd             │                   the view: Kanban, List, Graph, … (as now)               │
│     ▓▓░░░░░░░░         │                                                                           │
│   ▣ old-app (missing)  │                                                                           │
│                        │                                                                           │
├────────────────────────┴───────────────────────────────────────────────────────────────────────────┤
│ ● Up to date · 2 agents working       The atm app, continued: 3 of 6 done      1 problem · v0.35.0 │  status bar
└────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

On Windows and Linux only the title bar's row differs: the menus at its left, the window buttons at its right.

```text
┌────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ [=]  File  Edit  View  Go  Window  Help │ ▣ atm (2) × │ ▣ compmd × │ +                 _   □   ×   │
├────────────────────────┬───────────────────────────────────────────────────────────────────────────┤
```

`o o o` are macOS's window buttons, `[=]` the sidebar's button, `▣` a project's logo, `(2)` the needs-you count (on a
tab, in the sidebar, and the toolbar's chip, which lists them), `▸` the open project, `▓░` a sprint's progress.

The charts (progress, activity) move to the status bar (§5) and the Progress view; the version chip and the ⋯ menu go
to the menus (§6); the project and workspace selectors go to the sidebar (§4).

**On macOS** the window uses the overlay title bar (`titleBarStyle: "Overlay"`, "the title bar as a transparent
overlay over the window's content", and `hiddenTitle: true`; in the window's configuration and on
`WebviewWindowBuilder`): the traffic lights sit over the page's first row, and `trafficLightPosition` centres them in
it (it requires the overlay and `decorations: true`). macOS keeps its own zoom, tiling and full-screen behaviour.

**On Windows and Linux** the window has no decorations (`decorations: false`), and the page draws minimize, maximize
and close. They call the app's commands `minimize`, `toggle_maximize` (`is_maximized`, then `maximize` or
`unmaximize`: Tauri has no toggle) and `close_window` (`Window::close`, "like a user-initiated close request", so the
app's own close handling runs). Windows 11 gives undecorated windows rounded corners and a border with
`set_shadow(true)`.

**Dragging the window** by its bars: Tauri's `data-tauri-drag-region` calls the window API from the page, which needs
the core permission `core:window:allow-start-dragging`, ruled out by I25. Instead the app has its own command,
`start_dragging` (`Window::start_dragging`), which the bars call on a mouse-down outside their controls, and a double
click calls `toggle_maximize`. On Windows, Tauri's guide also gives the CSS `app-region: drag` for touch and pen.

**The window's title** (the taskbar, the window switcher, Mission Control) is the active tab's: the page sets
`document.title`, and in the app also calls `set_title` (`Window::set_title`).

**To check on Windows and Linux** (a spike in the title bar's task, on each platform):

- Windows 11's snap layouts, shown on hovering the maximize button: they come with the system's button, and Tauri's
  documentation doesn't say whether a page's button can have them;
- resizing an undecorated window from its edges, on Windows and on Linux (X11 and Wayland, where GNOME expects the app
  to draw its frame);
- the system's window menu (Alt+Space on Windows).

If one of them can't be had without a hack, the fallback is the system's title bar on that platform with the same two
rows under it: everything else stays the same.

## 3. Tabs

**Tabs drawn by the page, in the title bar, the same on all three.** A tab is an **address** (the page's, e.g.
`/projects/atm/kanban?task=T-12`): the window has one page and one dashboard state, and switching tabs navigates to the
tab's address, as a click on a project does today. So the dashboard's state stays one (`state.svelte.ts`), the
router, deep links and T-506's subscriptions work as they do, and a tab costs nothing while it isn't shown.

- **The tab strip:** each tab its project's logo and name (or the page's title), a close button, and the needs-you
  count of its project; drag to reorder; the **+**, ⌘T and File ▸ New Tab open the workspace's overview.
- **Switching** (a click, ⌃Tab and ⌃⇧Tab, ⌘⇧] and ⌘⇧[; ⌘1–⌘6 stay the views'): the page navigates to the tab's
  address.
  The project's data is loaded again through the commands (the same as switching projects now); the tab keeps what is
  in its address (the view, the open task or document, the sprint) and the page's preferences keep the rest.
  **To measure** in the tabs' task: a switch within the e2e budget (200 ms, `tests/e2e/performance.spec.ts`) for a
  large project; if not, the page keeps the last few projects' data, a cache in `state.svelte.ts`, not a state per tab.
- **Open in a new tab:** ⌘-click (Ctrl-click, a middle click) on a project in the sidebar, a task, a link to a document.
- **Close Tab** (⌘W) closes the tab; closing the last one closes the window.
- **Move Tab to New Window** (the tab's context menu, the Window menu): the app opens a window on the tab's address
  (`open_window`), and the tab leaves this one. Dragging a tab out of the window comes later, if wanted.
- **Kept per window:** the tabs, their order and the active one are the window's (§7 brings them back).

Rejected:

- **macOS's window tabs** (`tabbingIdentifier`): macOS only, so Windows and Linux would get windows instead of tabs.
  Without a tabbing identifier, "automatic tabbing will be disabled" (Tauri), so macOS adds no tab items of its own.
- **One webview per tab in one window** (`Window::add_child`): "available on desktop and crate feature `unstable`
  only", so not a stable API.
- **A state per tab** in the page: a rewrite of `state.svelte.ts` for a speed the measure above may show isn't needed.

**Several windows** stay possible (New Window, ⌘N; Move Tab to New Window), each with its own tabs. For them the app
changes:

- the windows are `main` and `window-<n>`; the capability (`capabilities/main.json`) names them (`"windows": ["main",
  "window-*"]`), and I25's tests check a `window-` window as `main`;
- **subscriptions belong to a window:** `AppState` keeps them by the webview's label, and a page loading again ends
  only its own window's (`unwatch_all` becomes `unwatch_window`); a window closing ends its own too;
- **deep links** open in the window in front (the last focused), in a new tab; with no window open, in a new window.
  The second instance focuses that window;
- `open_window(address)` checks the address with `link_address` (the deep links' check, T-459) before it makes the
  window.

## 4. The side panel

**A collapsible sidebar** listing the workspace's projects, in place of the project selector:

- at the very top, **search** (⌘F, Edit ▸ Find): the open project's tasks, as the search does now; on the overview,
  every project's (the app's `search` command);
- then the workspace (a menu, `WorkspaceSelector`'s content);
- then **Overview**, and each project: its logo, name, the active sprint's progress as a thin bar, and the needs-you
  count as a badge;
- the open project marked; a click opens it in this tab, ⌘-click in a new one (§3);
- missing projects (a folder without a task file) dimmed, as the overview shows them.

It is a Svelte component (`shell/Sidebar.svelte`), shown only in the app, from what the page already loads
(`dashboard.projects`, the overview's summaries). View ▸ Show Sidebar (⌃⌘S, Ctrl+B) and the title bar's sidebar button
toggle it, and dragging its edge resizes it; both are kept in the page's preferences (`lib/prefs.ts`). The browser
keeps its selectors. While it is open, the title bar's tabs start at its edge, over the page they show, and slide there
and back as it opens and closes (T-529; at once with reduced motion).

macOS's translucent sidebar (vibrancy) needs a transparent window and has no equal on the others; it is not proposed.

## 5. The status bar

**The footer becomes a status bar,** one line high (about 24 px), in the app only:

- **left:** live: "Up to date" while the app watches the project (T-506), or why not; the agents at work (from
  `claims`), as "2 agents working", a click opens the Agents view;
- **middle:** the active sprint, its name and progress (`ProgressWidget`'s numbers, in text);
- **right:** the archive and problems notices (as now), and the version, which opens What's new.

GitHub, Help, About and Connect your agents move to the menus (§6). The browser keeps its footer.

## 6. Menus and shortcuts

**One menu, defined once, in Rust** (`tauri::menu`, in `tauri` itself: no plugin): `Submenu`s of `MenuItem`s,
`CheckMenuItem`s for the toggles, `PredefinedMenuItem`s for the system's own (Services, Hide, Quit, Cut, Copy,
Paste, …), and `AboutMetadata` for About.

- **On macOS** it is the app's menu bar, at the top of the screen.
- **On Windows and Linux** the page draws the same menus in its title bar (File, Edit, View, Go, Window, Help, as
  VS Code does on both), from the menu's definition, which the page gets from the app (`menu`, a command returning the
  items, their labels, shortcuts and check marks). The native menu is attached to the window and hidden
  (`hide_menu`), so its accelerators still work. **To check** (with the title bar's spike): Tauri's accelerators fire
  while the menu is hidden; if they don't, the page handles those keys itself, from the same definition.

Either way an item sends the page an action on a channel, as the deep links do (`subscribe_links`): `subscribe_menu`
gives the page a channel of `MenuAction`s (a closed enum, generated into the page's types by tauri-specta), which the
page maps to what the same control does now (`dashboard.selectTab`, `openDialog`, `navigate`). The menu reaches only
the window in front.

| Menu | Items |
|---|---|
| **atm** (macOS; on Windows and Linux its items go to File and Help) | About atm · Check for Updates… · Settings… ⌘, · Connect Your Agents… · Services, Hide, Quit |
| **File** | New Tab ⌘T · New Window ⌘N · Close Tab ⌘W · Copy Link to This Page ⌥⌘C · (Windows, Linux: Settings…, Exit) |
| **Edit** | Undo, Cut, Copy, Paste, Select All · Find ⌘F (the sidebar's search) |
| **View** | Graph ⌘1 · Kanban ⌘2 · List ⌘3 · Agents ⌘4 · Progress ⌘5 · Project ⌘6 · Show Sidebar ⌃⌘S · Show Filters ⌥⌘F · Include Archived Tasks · Reload ⌘R · Enter Full Screen |
| **Go** | Back ⌘[ · Forward ⌘] · Projects Overview ⇧⌘O · each project of the workspace · Workspace ▸ (each workspace) |
| **Window** | Minimize, Zoom · Next Tab, Previous Tab · Move Tab to New Window · the open windows |
| **Help** | atm Help ⌘? · What's New · atm on GitHub · CompMD · (Windows, Linux: About atm, Check for Updates…) |

- **State the menu shows** (the current view checked, Show/Hide Sidebar, Include Archived): the page tells the app
  with one command, `menu_state`, when it changes, for its own window; the app updates the items when that window is in
  front. The Go menu's projects and workspaces come from `AppState`'s workspaces, in Rust.
- **Shortcuts are the menu's accelerators** (§1a), shown in the menu; on macOS remappable in System Settings. The
  page's own keys in the browser stay; in the app the page leaves the ones the menu has to it, so none fires twice.
- **Settings** (⌘,) opens the settings as a sheet over the window: the ⋯ menu's content (mode, theme, document style,
  reset the view), laid out as a settings pane, with the badge's switch (§7). The ⋯ popup stays in the browser; in
  the app the title bar's ⋯ opens the sheet too (T-517: easier to find than the menu alone). A settings window of its own would
  be a second page sharing the preferences through `localStorage` (one origin, so other windows see a change through
  the `storage` event); it is the next step if the sheet feels wrong.

## 7. Window state and the badge

The same on the three desktops.

- **Windows come back as they were:** each window's size and place, its tabs (their addresses and order) and the
  active one. The app writes the windows' list to a file in its own data folder when it changes, and opens them again
  at the start; a deep link it was started with opens in a new tab of the first. Tauri's `tauri-plugin-window-state`
  keeps frames but not tabs, so this design proposes no new plugin.
- **The badge:** the workspace's needs-you count (in review, blocked on you), off in Settings. On macOS and Linux
  `Window::set_badge_count` ("Using `0` or `None` will remove the badge"; Linux where the desktop shows one); on
  Windows, where Tauri says to use `Window::set_overlay_icon` instead, a small icon with the count over the taskbar
  button.
- **Not now, on any platform:** the Dock menu (macOS) and the jump list (Windows): Tauri 2.12 has no API for either
  (its menus are the app's, windows', context and tray menus), and doing it through the platform directly is the kind
  of hack this design rules out.

## 8. How it fits what is there

- **Addresses and deep links (T-459):** unchanged. A tab is an address; a deep link, a menu item and ⌘-click all end
  at the router's `navigate`, in this tab or a new one, or at `open_window` with an address the app checked.
- **T-506 (no refresh settings):** unchanged: one page per window, its subscriptions those of the tab shown. The
  status bar shows that the page is live; ⌘R reloads.
- **The updater (T-483):** Check for Updates… opens what `UpdateNotice` shows today.
- **The browser dashboard:** no change. The new components are in the app only; the shared ones (TitleBar, Footer)
  get an app variant beside their browser one. The browser's e2e specs and screenshots don't change.
- **Security:** new commands only, each in `build.rs`'s manifest with one permission (`shell`, `menu`,
  `subscribe_menu`, `menu_state`, `set_title`, `start_dragging`, `minimize`, `toggle_maximize`, `close_window`,
  `open_window`); no core or plugin permission. A window acts on itself only (the commands take the calling window,
  never a label). `open_window` takes an address and checks it as a deep link. I25's text gains the `window-` windows;
  `docs/security-architecture.md` is updated with the commands.

Checked against Tauri's documentation for 2.12.5 (2026-11-09): the window configuration
([v2.tauri.app/reference/config](https://v2.tauri.app/reference/config/), `WindowConfig`), the window customization
guide ([v2.tauri.app/learn/window-customization](https://v2.tauri.app/learn/window-customization/)), the window menu
guide ([v2.tauri.app/learn/window-menu](https://v2.tauri.app/learn/window-menu/)), and `WebviewWindowBuilder`,
`Window`, `AppHandle` and the `tauri::menu` module (docs.rs, `tauri` 2.12.5).

## 9. Steps (the tasks this becomes)

Each a task of its own, in this order; each checked in a window both ways on macOS (`cargo tauri dev`, `cargo run -p
atm-app`), with the app e2e specs (`tests/e2e/app.spec.ts`). The Windows and Linux checks (§2's spike, the menus drawn
by the page) need those platforms: they run in CI's builds once T-482 is done, and by hand on each before the app
ships there.

1. **Windows and the shell:** subscriptions by window, deep links and the second instance to the window in front,
   `set_title`, `shell`, `open_window`, the capability for `window-*`. No visible change yet.
2. **The menus:** the definition of §6, macOS's menu bar, `subscribe_menu`, `menu` and `menu_state`, the shortcuts;
   GitHub, Help, About, Connect your agents and What's new reachable from it.
3. **The title bar:** the two rows of §2, macOS's overlay, the page's window buttons and menus for Windows and Linux,
   the window commands; the spike on Windows and Linux.
4. **Tabs** (§3): the strip, tabs as addresses, New Tab, Close Tab, switching, ⌘-click, Move Tab to New Window; the
   switch measured.
5. **The sidebar** (§4).
6. **The status bar** (§5), the footer's links gone in the app.
7. **Settings as a sheet** (§6).
8. **Windows come back, and the badge** (§7).

## 10. Decisions (Hakan, 2026-11-09)

1. **Tabs are addresses** in one page (§3), a project loaded again on a switch (measured), not a state per tab.
2. **The page's own window buttons and menus on Windows and Linux** (§2, §6), with the system's title bar as the
   fallback if the spike finds a hack is needed.
3. **The view switcher in the toolbar** (§2).
4. **Settings as a sheet** first (§6); a window of its own only if the sheet feels wrong.
5. **The badge stays** (§7). Notifications for what needs you need a plugin (`tauri-plugin-notification`), so they are
   a later task of their own, not part of these steps.
