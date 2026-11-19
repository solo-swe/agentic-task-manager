# Tabs, pop-ups, Back and Forward, and an address for every page (ATM-653)

**Status:** research and design, for review with Hakan (CR). Nothing is built until the design is agreed; the build
becomes tasks of its own (§7).

Hakan (2026-11-19): the way the app uses tabs is inconsistent. Back after opening a review always lands on the Reviews
list, wherever the review was opened from. It is unclear whether a pop-up belongs to a tab or to the app, what the
right-click menu's Back and Reload do, and what ⌘/Ctrl-click does. Every page should have a proper address.

This replaces the tab parts of [the desktop UI design](app-desktop-ui-design.md) §3 (a tab is an address, one state,
no per-tab state) once agreed; the rest of that document stands.

## 1. In short

- **Today a tab is only an address, and the window has one history.** All title-bar tabs share the browser history
  of one page. Switching tabs pushes an entry, so Back walks across tabs and rewrites the active tab with another
  tab's address. The review page's **← Reviews** is a link to the list, not Back. Together these are why Back "always"
  lands on the Reviews list.
- **The model proposed, as the browsers have it:** a tab is a place with its **own history**. Back and Forward walk
  that tab's history only, in the order the reader went. Each history entry is a full address: the page and
  everything open over it (task details, a document, a review's file and round). A reload, a new tab or a link from
  outside lands on exactly that.
- **Pop-ups come in three kinds:**
  1. **places over a page** (task details, a document): in the address and the tab's history;
  2. **questions** (a form, a confirmation, a review being submitted): belong to their tab, and stay on it while you
     switch tabs, as Safari's sheets and Chrome's dialogs do;
  3. **the app's own** (Settings, Help, What's new, About): above every tab, in no tab's history.

  So yes: you can switch tabs with a pop-up open on one, and it is still there when you come back.
- **Clicks, as everywhere else:**
  - a click opens in place;
  - ⌘/Ctrl-click or a middle click opens a new tab in the background;
  - ⇧⌘-click opens a new tab and goes to it;
  - right-click is atm's own menu, whose Back and Reload act on the tab.

  A plain click on a review no longer opens a new tab.

## 2. How others do it

| | What a tab is | Pop-ups belong to | Back and Forward | ⌘/Ctrl-click, middle click | A page's address |
|---|---|---|---|---|---|
| **Chrome, Safari** | A place, with its own history | JavaScript dialogs belong to their tab; since 2017 Chrome dismisses one when its tab is left, and a background tab's waits behind an indicator [^chrome] | The tab's own history | A new tab in the background; ⇧ brings it forward | The URL, every entry |
| **Arc** | As the browsers'; tabs live in a sidebar and archive themselves | As Chrome's | The tab's own | As Chrome's | The URL |
| **macOS document tabs** (Pages, TextEdit, Finder) | A document's window, gathered as a tab | A **sheet** is document-modal: it blocks its own document, not the others, so the other tabs can be used while it is open [^sheets] | Finder: per window (tab) | Finder: ⌘-double-click opens a folder in a new tab | The document's file |
| **VS Code, Zed** | An open editor (a document) | The command palette and dialogs belong to the window | **Go Back** walks a history of places across editors, scoped to the window by default; since 1.65 it can be scoped to the editor group or the editor [^vscode] | Opens to the side | The file and position |
| **Slack** | No tabs | The window | One history of channels and conversations visited, ⌘[ and ⌘] [^slack] | Opens a thread or link in the side panel | A link per message |
| **Linear** (desktop) | As a browser's: pinned tabs, dragged to reorder, ⌘T [^linear] | The tab (a modal over its issue) | The tab's own | ⌘/Ctrl-click opens a new tab; right-click doesn't offer it [^linear-nav] | A URL for every issue, view and project |
| **Notion** (desktop) | As a browser's, tabs kept across restarts | The tab | Per tab | ⌘/Ctrl-click opens a new tab, no longer a window [^notion] | A URL per page, a block's anchor |
| **Figma** (desktop) | A file (and Home) | The tab | The file's own | ⌘-double-click opens a file in a new tab; the file browser's menu has *Open in new tab* [^figma] | A URL per file, node and page |

Claims with a footnote are sourced; the rest (the browsers' and Arc's clicks and histories, Finder, Zed) describe the
apps as they are used.

The tools closest to atm (Linear, Notion) copy the browser: a tab is a place with its own history, ⌘-click opens a new
tab, every page has a URL, and a modal belongs to the page it was opened on. VS Code differs because its tabs are
documents, not places. atm's tabs are places (a board, a project, a review), so the browser's model fits.

## 3. atm today

How it works now, mapped from the code.

**Tabs:**
- **Two kinds of tabs.**
  - **The title bar's tabs**, in the app only (`app-tabs.svelte.ts`, `TitleBarTabs.svelte`). A tab is
    `{ id, address }`, its label taken from its address. The window has one page, one dashboard state and **one
    browser history**.
  - **The view tabs** (Kanban, List, Graph, Progress, Agents, Project, Reviews), a path segment of the address. Choosing
    one pushes an entry.
- **Switching a title-bar tab** navigates the one page to that tab's address and **pushes** a history entry
  (`tab-actions.ts`: `navigate(address)`). The tab keeps nothing else: no scroll, no review sub-tab, and nothing open
  that has no address.

**Back:**
- **Back and Forward** (⌘[ ⌘], Go menu, `history.back()`) walk the window's one history. After a tab switch, Back goes
  to the previous tab's last address and shows it **in the current tab** (`appTabs.follow` rewrites the active tab to
  wherever the page went).
- **The review page's ← Reviews** (`ReviewsTab.svelte`: `onback={() => go(listAddress)}`) pushes the project's Reviews
  list. It never goes back: opened from a Kanban card or a task's details, it still lands on the list.

**Opening things:**
- **A review opens in a new tab on a plain click** in the app (`ReviewLink.svelte`, `ReviewsTab.svelte`,
  `reviews/open.ts`), in place in the browser. ⌘/Ctrl or a middle click opens a new tab on task cards, rows, document
  links and sidebar projects (`new-tab-clicks.ts`). A middle click on a title-bar tab **closes** it. In the browser,
  task cards have no ⌘-click handling.
- **Right-click** has no handler anywhere: the app shows WebKit's own menu. Its Reload reloads the page outside the app's
  own reload (`reload_window`, which exists because the address is a route the app has no file for). Its Back is the
  window's one history, as above.

**What has an address:**

| Address | Has none |
|---|---|
| `/`, `/projects`, `/board`, `/progress`, `/agents`, `/reviews` | Settings, What's new, the filters panel |
| `/projects/<id>/<view>`, `/projects/<id>/reviews/<CR>` | A review's sub-tab (Overview, Diffs, Comments) |
| `?task=`, `?doc=path:L40`, `?sprint=` | A selected comment or chain |
| `?about`, `?help`, `?agents` | A task peeked on the board of all projects |
| a review's `round`, `against`, `file` | The Reviews list's filter, a document's scroll |

**Pop-ups:**
- They are mounted once per window (`App.svelte`): task details, the document viewer, Help, About, Agents, What's new,
  Settings.
- On a tab switch, those with an address follow the new tab's address. Settings, What's new and the filters, which
  have none, **stay open over the other tab**.
- Closing task details or a dialog *replaces* its history entry, which leaves the identical entry before it. Back
  after closing then seems to do nothing.

**Other mismatches:**
- The desktop UI design says *Move Tab to New Window* is on a tab's context menu (none exists).
- It also says ⌘⇧[ and ⌘⇧] switch tabs; the menu has Ctrl+Tab, and ⌘[ ⌘] are Back and Forward.
- `docs/multi-project-api.md` says a review page has no title bar or tabs; in the app it has both.

## 4. The model for atm

### 4.1 A tab

**A tab is a place in atm with its own history.** In the browser, a browser tab is one; in the app, a title-bar tab is
one. Each keeps:

- **its history:** the addresses it went through, in order, and where it is in them;
- **per entry:** the full address (§4.4) and the scroll of the page it shows, restored on Back and Forward;
- **what is open on it that has no address:** a question being answered (§4.2), a draft comment.

A tab doesn't keep app-wide things (Settings, the theme). Switching tabs adds **nothing** to any history: it shows the
other tab as it was left.

**One page, many tabs (the app).** The window keeps one page, so a tab's history is atm's own, kept per tab, not the
browser's:
- the page's `history` is driven to stay one entry deep;
- ⌘[ ⌘], the menu's Back and Forward, the mouse's back button and a swipe all go to the active tab's history.

The alternative, a webview per tab, gives each tab the browser's history for free but costs a page (memory, a load)
per tab. It is an open question (§6.3); the model is the same either way.

**The view tabs and a review's sub-tabs are places, not tabs.** Kanban, List and the others, and a review's Overview,
Diffs and Comments, are pages within a tab: each has an address, and choosing one is a step in the tab's history, as
GitHub's pull request tabs are.

### 4.2 Pop-ups

| Kind | Examples | Belongs to | In the address and history | On a tab switch | On a reload |
|---|---|---|---|---|---|
| **A place over a page** | Task details, a document, a review's comment in view | The tab | Yes: opening one is a step (Back closes it); closing one goes back to the step before it when that is the page under it, else replaces | Hidden with its tab; there again on return | Restored |
| **A question** | Add task, a confirmation, Submit review, a dropdown or a menu | The tab (as a sheet in macOS, a dialog in Chrome) | No | Kept on its tab, hidden; a menu or dropdown closes (it is a moment, not a task) | Gone; what was typed is kept as a draft where atm keeps drafts (comments) |
| **The app's own** | Settings, Help, What's new, About, the search | The app, above every tab | No: Help and About lose their `?help` and `?about` (a link to Help stays, as `atm:` link and Help menu) | Stays open | Gone |

So a review being submitted on one tab doesn't stop the reader looking at another tab, and is there when they come
back.

### 4.3 Back and Forward

- **The tab's history only**, in the order the reader went. A review opened from a Kanban card goes Back to the board;
  from a task's details, to the board with those details open; from the Reviews list, to the list.
- **The page's own back button is Back, saying where it goes:** a review page's button reads **← Board**, **← ATM-633**
  or **← Reviews**, the title of the entry before it, and goes back to it. When there is no entry before it (a tab
  opened on the review, a link from outside), it reads **← Reviews** and goes up to the project's list, replacing the
  entry. This is iOS's navigation bar, and what Hakan expected.
- **Closing a place over a page** goes Back when the entry before it is the page under it, so Back after a close
  doesn't land on the same page twice.
- **A new tab** starts its own history with the address it was opened on. Its Back is disabled until it moves.

### 4.4 An address for everything a tab shows

| What | Address (proposed) | Today |
|---|---|---|
| A review, its sub-tab | `/projects/<id>/reviews/<CR>` (Overview), `…/<CR>/diffs`, `…/<CR>/comments` | Overview only; the sub-tab isn't kept |
| Its file, round, the round against | `…/diffs?file=&round=&against=` | As now |
| A comment in view | `…/diffs?file=…#c12` (a chain's anchor) | None |
| A task's details over any page | `?task=ATM-633` | As now |
| A document over any page | `?doc=path:L40` | As now |
| The Reviews list's filter and search | `/projects/<id>/reviews?status=waiting&q=…` | Kept in the browser only |
| A sprint shown | `?sprint=` | As now |
| Settings, Help, What's new, About | None (the app's own) | `?help`, `?about` |

Every link in atm is a real link (`<a href>` with this address), so ⌘-click, a middle click, *Copy Link* and *Open in
New Tab* work the same on every card, row and reference, in the browser for free and in the app through §4.5.

### 4.5 Clicks and menus

| Action | Browser | App |
|---|---|---|
| Click on a link, card or row | In place, a step in the tab's history | Same (a review too: no new tab on a plain click) |
| ⌘/Ctrl-click, middle click | The browser's: a new browser tab in the background | A new title-bar tab in the background, right of this one |
| ⇧⌘-click | The browser's: a new tab, brought forward | A new tab, brought forward |
| Middle click on a title-bar tab | — | Closes it (as browsers do; the one place a middle click closes) |
| Right-click on a link, card or row | The browser's menu | atm's: **Open**, **Open in New Tab**, **Open in New Window**, **Copy Link** |
| Right-click on a title-bar tab | — | **New Tab**, **Duplicate**, **Move to New Window**, **Copy Link**, **Close**, **Close Other Tabs** |
| Right-click elsewhere | The browser's | atm's: **Back**, **Forward**, **Reload** (this tab), and the system's text items over text |
| ⌘[ ⌘], mouse back and forward, a swipe | The browser's (the tab's) | The tab's history |
| ⌘R, Reload | The browser's | Reloads the page and shows this tab where it was, not the window's first tab |

## 5. What changes

| Where | Change |
|---|---|
| `app-tabs.svelte.ts`, `tab-actions.ts` | A tab has its own history and the scroll of each entry; switching tabs pushes nothing. Back and Forward act on the active tab |
| `navigation.ts` | `navigate` adds to the active tab's history; in the app the page's own history stays one entry deep |
| `ReviewsTab.svelte`, `ReviewPage.svelte` | The back button goes Back, labelled with the entry before it; Up to the list only when there is none |
| `ReviewLink.svelte`, `reviews/open.ts`, `OverviewReviews.svelte` | A plain click opens a review in place; ⌘-click a new tab |
| `state.svelte.ts` | Closing task details or a document goes Back when that lands on the page under it. A review's sub-tab, the Reviews list's filter and a comment in view join the address. Help and About leave it |
| `App.svelte`, the dialogs | Questions belong to their tab: kept and hidden on a switch. App-wide ones stay above every tab |
| A new context menu (`ui/`, the app only) | Right-click menus of §4.5; WebKit's default turned off in the app |
| `new-tab-clicks.ts`, the cards and rows | Every card and row is a real link; ⌘-click and middle click handled once, in one place |
| Docs | `app-desktop-ui-design.md` §3 and §6 replaced by this; `multi-project-api.md` §2.7's addresses |

## 6. Questions for Hakan

1. **The review page's back button:** labelled with where it goes, and Back (recommended)? Or always Up to the Reviews
   list, with Back only from ⌘[ and the menu?
2. **Questions on a tab switch:** kept on their tab (recommended, as Safari and Chrome do), or closed?
3. **One page for all tabs, or a webview per tab:** one page with atm keeping each tab's history (recommended: it is how
   the app works now, and costs nothing per tab), or a webview per tab (the browser's history for free, a page per
   tab)?
4. **Help and About:** app-wide, without an address (recommended), or kept as `?help` and `?about`?

## 7. Tasks it implies

1. **A tab's own history:** per-tab history and scroll in the app; a switch adds nothing; Back and Forward (keys,
   menu, mouse, swipe) act on the tab. Tests: the three traces of §4.3 in the app and in the browser.
2. **The back button is Back:** labelled with the entry before it; Up when there is none. Closing a place over a page
   goes Back.
3. **The rest of the addresses:** a review's sub-tab, a comment in view, the Reviews list's filter; Help and About out of
   it.
4. **Clicks:** a plain click on a review in place; every card and row a real link; ⌘/⇧⌘/middle click handled once.
5. **Context menus** in the app: on links, on tabs, elsewhere; WebKit's turned off.
6. **Pop-ups by kind:** questions kept on their tab, app-wide ones above every tab.
7. **Docs:** the desktop UI design and the addresses table brought in line.

[^chrome]: Chrome for Developers, [History of JavaScript dialogs](https://developer.chrome.com/blog/dialogs-policy).
[^sheets]: Apple, [Human Interface Guidelines: Sheets](https://developer.apple.com/design/human-interface-guidelines/sheets) and [Introduction to Sheets](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Sheets/Sheets.html) (a sheet is document-modal).
[^vscode]: InfoWorld, [Visual Studio Code 1.65 overhauls editor history navigation](https://www.infoworld.com/article/2334593/visual-studio-code-165-overhauls-editor-history-navigation.html) (`workbench.editor.navigationScope`).
[^slack]: Slack Help Center, [Slack keyboard shortcuts](https://slack.com/help/articles/201374536).
[^linear]: Linear, [Changelog](https://linear.app/changelog/page/12) (tabs in the desktop app).
[^linear-nav]: Linear, [Navigating Linear](https://linear.app/enablement/guide/navigating-linear).
[^notion]: Notion Help Center, [New tabs in the Notion desktop app](https://www.notion.com/help/new-tabs-in-the-notion-desktop-app); Notion, [Release notes, 2022-12-15](https://www.notion.com/en-gb/releases/2022-12-15).
[^figma]: Figma Forum, [Allow me to search and open existing files in new tab in the Figma app](https://forum.figma.com/t/allow-me-to-search-and-open-existing-files-in-new-tab-in-the-figma-app/12381/21).
