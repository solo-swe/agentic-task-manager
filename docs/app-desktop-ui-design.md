# The atm app as a desktop app

Status: **draft** (T-510), for review with Hakan. Once agreed, `/atm-create-tasks` breaks §9 into tasks.

The atm app (T-455 onwards, the [Tauri guide](guides/tauri.md)) shows the dashboard's page as a browser shows it: a
header with the title, the selectors and the ⋯ menu, and a footer of links, in a plain window. This document says what
changes to make it a desktop app, and what stays the same as the browser dashboard.

What this aims for:

- **One Svelte code base:** the browser dashboard keeps working as it does, and its e2e suite and screenshots don't
  change. The app's shell is a variant of the page's shell, switched on `inApp` (`lib/api/http.ts`); the views,
  dialogs, state and addresses are shared.
- **Native on each platform:** the menu bar, the title bar, tabs and keyboard shortcuts behave as in the platform's
  other apps. Where macOS has a system feature the others don't (window tabs), macOS uses it and the others get their
  nearest one, never a web imitation of macOS (§1a).
- **Only Tauri's documented APIs,** and no new plugin without a yes (the [Rust guide](guides/rust.md)).
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

The app ships on macOS now. Windows and Linux come next (their tests in T-482, the deep-link plugin when the app ships
there; the [Tauri guide](guides/tauri.md)). iOS and Android aren't planned, and nothing here stands in their way.
Every part of this design is for all three desktop platforms; what differs is where a platform has its own way:

| Part | macOS | Windows | Linux | iOS, Android |
|---|---|---|---|---|
| Title bar (§2) | the page's bar, under the overlay title bar (the window buttons over it) | the native title bar, the page's bar as a toolbar under it | the same as Windows (the window manager's title bar) | none: the page's bar at the top |
| Tabs (§3) | the system's window tabs | windows: New Tab opens a window, the Window menu lists them | the same as Windows | none: one page, the sidebar switches projects |
| Menus (§6) | the menu bar at the top of the screen | the same menu, in each window under its title bar | the same as Windows | none (Tauri's menu API is desktop only): the page's ⋯ menu and footer links stay |
| Shortcuts (§6) | ⌘ | Ctrl | Ctrl | the page's own keys, as in the browser |
| Sidebar, status bar, settings sheet (§4–§6) | yes | yes | yes | the browser's header and footer |
| Windows come back (§7) | yes | yes | yes | not needed: one window |
| Badge (§7) | the Dock's | none yet (Tauri: `set_overlay_icon` instead) | the launcher's, where the desktop shows one | not now |

From Tauri's documentation: menus "appear as part of the application window for Windows or Linux, or in the menu bar
on MacOS", and the menu API is "available on desktop only"; the overlay title bar, the hidden title and the tabbing
identifier are macOS only.

**On Windows, no title bar of our own.** Drawing the whole bar there (`decorations: false`) means drawing the minimize,
maximize and close buttons too, and the page's buttons lose what Windows gives its own (the snap layouts on hovering
maximize, Windows 11). The native bar and a toolbar under it is what Windows apps without a custom frame do.

**The page asks about features, not about the OS.** One command, `shell`, says what the window has:
`{ menu_bar, overlay_title_bar, system_tabs }` (from `cfg!(target_os)` and the build), and the shell's app variant
follows it: no menu bar, so the ⋯ menu and the footer's links stay; no overlay, so the bar is a toolbar; no system
tabs, so New Tab is a window. In the browser, none of them.

**Shortcuts are written once,** as `CmdOrCtrl+…` in the menu's accelerators (Tauri's menus map `CmdOrCtrl` to ⌘ on
macOS and Ctrl on Windows and Linux). Where the platforms' conventions differ, each gets its own:

| Action | macOS | Windows, Linux |
|---|---|---|
| Back, Forward | ⌘[, ⌘] | Alt+←, Alt+→ |
| Show Sidebar | ⌃⌘S | Ctrl+B |
| Full Screen | ⌃⌘F | F11 |
| Help | ⌘? | F1 |
| Quit | ⌘Q | Ctrl+Q on Linux; Alt+F4 (the system's) on Windows |
| Open in a new tab | ⌘-click | Ctrl-click; a middle click on all three |

The rest (⌘T, ⌘N, ⌘W, ⌘,, ⌘F, ⌘1–⌘6, ⌘R) are the same keys with Ctrl. The tables below give the macOS keys.

## 2. The title bar

**The page's header becomes the window's title bar.** The window uses Tauri's overlay title bar on macOS
(`titleBarStyle: "Overlay"`, "the title bar as a transparent overlay over the window's content", and
`hiddenTitle: true`; both macOS only, in the window's configuration and on `WebviewWindowBuilder`): the traffic
lights sit over the page, and the page draws the bar. `trafficLightPosition` moves them to the bar's middle (it
requires the overlay and `decorations: true`).
In the app, `TitleBar.svelte` becomes one row, as tall as a toolbar title bar (about 52 px):

- **left, after the traffic lights:** the project's logo and name (or "Projects Overview"), the active sprint as a
  subtitle in small text;
- **centre:** the view switcher (Graph, Kanban, List, …) as a segmented control, which is where Mac apps put it;
- **right:** search, the Filters popup, the needs-you chip, and the reload button.

The charts (progress, activity) move out of the bar into the status bar (§5) and the Progress view; the version chip
and the ⋯ menu go to the menu bar (§6); the project and workspace selectors go to the side panel (§4).

**Dragging the window** by its bar: Tauri's `data-tauri-drag-region` calls the window API from the page, which needs a
core permission (`core:window:allow-start-dragging`) that I25 rules out. Instead, the app has its own command,
`start_dragging`, that drags the window that called it (`Window::start_dragging`), and the bar calls it on a
mouse-down outside its controls; a double click calls `toggle_maximize`, which zooms the window or restores it
(Tauri has no toggle: `is_maximized`, then `maximize` or `unmaximize`). Two commands, in `build.rs`'s manifest, one
permission each; I25 keeps its wording.

**The window's title** (the Window menu, Mission Control, a tab's label) is the page's title: the page already sets
`document.title`; in the app it also calls `set_title`, a command that sets its own window's title
(`Window::set_title`).

Windows and Linux keep their native title bars (the overlay is macOS's); the page's bar is then a toolbar under it,
with the same row, and no `start_dragging` (the native bar drags the window), §1a.

## 3. Tabs

**macOS's own window tabs, one window per tab.** Each window gets the same tabbing identifier (`tabbingIdentifier`
in the configuration, `WebviewWindowBuilder::tabbing_identifier` for the windows the app makes; macOS only:
"Windows with matching tabbing identifiers will be grouped together"), so macOS groups them as tabs: the tab bar,
dragging a tab out into a window of its own, Merge All Windows and Show All Tabs come from the system, as in Finder or
Safari. New Tab (⌘T) is the app's own menu item (§6). Each tab is a whole page with its own address, state and subscriptions.

The alternative, tabs drawn inside one page, would need the dashboard's state (`state.svelte.ts`, one `dashboard`)
to become one per tab, the views kept alive or reloaded on every switch, and the tab strip, its keyboard handling and
dragging written by hand. That is a rewrite of the page for something macOS gives for free, so it is not proposed.

What one window per tab changes in the app:

- **Several windows** (`main`, then `tab-<n>`), reversing "one window" (T-457) for tabs and windows, while a window
  still switches workspaces in place. The capability (`capabilities/main.json`) names them all (`"windows": ["main",
  "tab-*"]`), and I25's tests check a tab window as `main`.
- **Subscriptions belong to a window:** `AppState` keeps them by the webview's label, and a page loading again ends
  only its own window's (`unwatch_all` becomes `unwatch_window`). A window closing ends its own too.
- **Deep links** open in the window in front (the last focused); with no window open, in a new one. The second
  instance focuses that window too.
- **New Tab** (⌘T, Ctrl+T) opens the workspace's overview; **⌘-click** on a project or a task (Ctrl-click on Windows
  and Linux, a middle click on all three) opens it in a new tab: the page
  calls `open_tab(address)`, which the app checks with `link_address` (the deep links' check, T-459) before it makes
  the window.
- **Memory:** each tab is a WKWebView with its own content process, about as heavy as a Safari tab; fine for the
  handful of projects a user keeps open.

**Windows and Linux have no system tabs:** there the same windows stay windows. New Tab opens a window, Ctrl-click
opens one, and the Window menu lists them, as apps without tabs of their own do on those platforms. The code is the
same: a window per page everywhere, and only macOS groups them. Tabs drawn by the page on every platform are the
alternative (above), at the cost of that rewrite; §10 asks.

**To check first** (a spike in the first task): the tab bar's **+** button. macOS shows it only for a window that
answers `newWindowForTab:`, which Tauri's API doesn't mention; without it, tabs open with ⌘T and File ▸ New Tab, and
the bar has no +. If the + matters, that comes back to the design (no hacks).

## 4. The side panel

**A collapsible sidebar** listing the workspace's projects, in place of the project selector:

- at the top, the workspace (a menu, `WorkspaceSelector`'s content);
- then **Overview**, and each project: its logo, name, the active sprint's progress as a thin bar, and the needs-you
  count as a badge;
- the open project marked; a click opens it in this tab, ⌘-click in a new one (§3);
- missing projects (a folder without a task file) dimmed, as the overview shows them.

It is a Svelte component (`shell/Sidebar.svelte`), shown only in the app, from what the page already loads
(`dashboard.projects`, the overview's summaries). View ▸ Show Sidebar (⌃⌘S, as in Finder and Xcode) toggles it, and
dragging its edge resizes it; both are kept in the page's preferences (`lib/prefs.ts`). The browser keeps its
selectors.

The translucent sidebar material of macOS (vibrancy) needs a transparent window, which changes how every theme paints
its background; it is left for later (§10).

## 5. The status bar

**The footer becomes a status bar,** one line high (about 24 px), in the app only:

- **left:** live: "Up to date" while the app watches the project (T-506), or why not; the agents at work (from
  `claims`), as "2 agents working", a click opens the Agents view;
- **middle:** the active sprint, its name and progress (`ProgressWidget`'s numbers, in text);
- **right:** the archive and problems notices (as now), and the version, which opens What's new.

GitHub, Help, About and Connect your agents move to the menu bar (§6). The browser keeps its footer.

## 6. Menus and shortcuts

**The app's own menu bar,** built in Rust with Tauri's menu API (`tauri::menu`, in `tauri` itself: no plugin):
`Submenu`s of `MenuItem`s, `CheckMenuItem`s for the toggles, `PredefinedMenuItem`s for the system's own (Services,
Hide, Quit, Cut, Copy, Paste, …), and `AboutMetadata` for About. Menu
items send the page an action on a channel, as the deep links do (`subscribe_links`): a new `subscribe_menu` command
gives the page a channel of `MenuAction`s (a closed enum, generated into the page's types by tauri-specta), which
the page maps to what the same control does now (`dashboard.selectTab`, `openDialog`, `navigate`). The menu reaches
only the window in front.

| Menu | Items |
|---|---|
| **atm** | About atm · Check for Updates… · Settings… ⌘, · Connect Your Agents… · the standard Services, Hide, Quit |
| **File** | New Tab ⌘T · New Window ⌘N · Close Tab ⌘W · Copy Link to This Page ⌥⌘C |
| **Edit** | the standard ones (Undo, Cut, Copy, Paste, Select All) · Find ⌘F (the search) |
| **View** | Graph ⌘1 · Kanban ⌘2 · List ⌘3 · Agents ⌘4 · Progress ⌘5 · Project ⌘6 · Show Sidebar ⌃⌘S · Show Filters ⌥⌘F · Include Archived Tasks · Reload ⌘R · Enter Full Screen |
| **Go** | Back ⌘[ · Forward ⌘] · Projects Overview ⇧⌘O · each project of the workspace · Workspace ▸ (each workspace) |
| **Window** | the standard ones (Minimize, Zoom); macOS adds the tab items (Show Tab Bar, Show All Tabs, Move Tab to New Window, Merge All Windows) to the submenu with Tauri's `WINDOW_SUBMENU_ID` |
| **Help** | atm Help ⌘? · What's New · atm on GitHub · CompMD |

- **State the menu shows** (the current view checked, Show/Hide Sidebar, Include Archived): the page tells the app
  with one command, `menu_state`, when it changes, for its own window; the app updates the items when that window is in
  front. The Go menu's projects and workspaces come from `AppState`'s workspaces, in Rust.
- **Shortcuts are the menu's accelerators**, written as `CmdOrCtrl+…`, so they work as the platform's do (shown in
  the menu; on macOS, remappable in System Settings), with each platform's own keys where they differ (§1a). The page's own keys in the browser stay; in the app the page leaves the ones the menu has to it, so none
  fires twice.
- **Settings** (⌘,) opens the settings as a sheet over the window: the ⋯ menu's content (mode, theme, document style,
  reset the view), laid out as a settings pane. The ⋯ menu stays in the browser. A settings window of its own would
  be a second page sharing the preferences through `localStorage` (one origin, so other windows see a change through
  the `storage` event); it is the next step if the sheet feels wrong.

## 7. Window state and the Dock

Window state is for the three desktop platforms alike (Tauri's window frame and position APIs are desktop's); the Dock
is macOS's, and Windows' and Linux's nearest things are noted with each item. On iOS and Android there is one window,
so nothing here applies (§1a).

- **Windows come back as they were:** their size and place, and their tabs with each one's address. The app writes the
  windows' list (label, frame, address) to a file in its own data folder when it changes, and opens them again at the
  start; a deep link it was started with opens in front of them. Tauri's `tauri-plugin-window-state` keeps frames but
  not addresses or tabs, so it doesn't save much over doing both ourselves; this design proposes no new plugin.
- **The Dock menu: not now** (nor Windows' jump list, the taskbar's menu: no Tauri API either). Tauri 2.12 has no API
  for it (its menu module has app, window, context and tray menus,
  none for the Dock), and doing it through AppKit directly is the kind of hack this design rules out. It waits for
  Tauri; nothing else depends on it.
- **The Dock badge:** the needs-you count of the workspace (in review, blocked on you), off in Settings, through
  `Window::set_badge_count` ("Using `0` or `None` will remove the badge"; macOS and Linux, not Windows, where Tauri
  points to `set_overlay_icon`, a small icon over the taskbar button: a later step for Windows, not this design's).

## 8. How it fits what is there

- **Addresses and deep links (T-459):** unchanged. A tab's address is the page's address; a deep link, a menu item
  and ⌘-click all end at the router's `navigate`, or at `open_tab` with an address the app checked.
- **T-506 (no refresh settings):** unchanged in each window. The status bar shows that the page is live instead of a
  refresh setting; ⌘R reloads.
- **The updater (T-483):** Check for Updates… in the atm menu opens what `UpdateNotice` shows today.
- **The browser dashboard:** no change. The new components are in the app only; the shared ones (TitleBar, Footer)
  get an app variant beside their browser one. The browser's e2e specs and screenshots don't change.
- **Security:** new commands only, each in `build.rs`'s manifest with one permission (`shell`, `start_dragging`,
  `toggle_maximize`, `set_title`, `open_tab`, `subscribe_menu`, `menu_state`); no core or plugin permission. A window
  acts on itself only (the commands take the calling window, never a label). `open_tab` takes an address and checks it
  as a deep link. I25's text gains the tab windows; `docs/security-architecture.md` is updated with the commands.

Checked against Tauri's documentation for 2.12.5 (2026-11-09): the window configuration
([v2.tauri.app/reference/config](https://v2.tauri.app/reference/config/), `WindowConfig`), `WebviewWindowBuilder`
and `Window` (docs.rs, `tauri` 2.12.5), and the `tauri::menu` module.

## 9. Steps (the tasks this becomes)

Each a task of its own, in this order; each checked in a window both ways (`cargo tauri dev`, `cargo run -p
atm-app`), with the app e2e specs (`tests/e2e/app.spec.ts`).

1. **Windows, not a window:** subscriptions by window, deep links and the second instance to the window in front,
   `set_title`, `shell` (§1a), the capability for `tab-*`; the spike on tabbing (§3). No visible change yet.
2. **The menu bar:** the menus of §6, `subscribe_menu` and `menu_state`, the shortcuts; GitHub, Help, About, Connect
   your agents and What's new reachable from it.
3. **Tabs and windows:** New Tab, New Window, Close Tab, ⌘-click, `open_tab`.
4. **The title bar:** the overlay, the one-row bar of §2, `start_dragging` and `toggle_maximize`.
5. **The sidebar** (§4).
6. **The status bar** (§5), the footer's links gone in the app.
7. **Settings as a sheet** (§6).
8. **Windows come back** (§7), and the Dock badge.

## 10. Questions for Hakan

1. **Tabs as macOS window tabs** (§3), one page per tab, with windows instead of tabs on Windows and Linux (§1a);
   or tabs drawn in the page, the same on every platform, at the cost of one state per tab?
2. **The view switcher in the title bar** (§2), or the views kept as tabs in the page under it?
3. **Settings as a sheet** first, or a window of its own from the start (§6)?
4. **Window state without the plugin** (§7), as proposed?
5. **Anything to drop or add:** the Dock badge, the translucent sidebar (later), notifications for what needs you
   (a plugin, `tauri-plugin-notification`, so a question of its own)?
