+++
id = "01a20e38-c64c-7203-b5b1-11af51b45fe0"
number = 160
title = "Tab titles: <scope> - <item>, with the item's icon"
status = "approved"
tasks = ["ATM-754"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T15:42:11Z
updated = 2026-11-24T16:35:59Z
+++

**Round 2:** one more test fixed (`0dec7e4`). The app's timed tab-switching test (`performance.spec.ts`, T-525) waited for `#dashboard-title` to be exactly the project's name, so it timed out on the new titles. My first round's e2e rerun didn't reach the timed project, which is how I missed it. It now waits for the title to start with the tab's scope (`Performance Pro… - `). The timed performance specs pass (10 of 10, twice each).

An app tab is now titled `<scope> - <item>` with the item's icon (ATM-754, [the design §3](docs/sidebar-overhaul-design.md#:~:text=A%20tab's%20title%20is)): `ATM - Tasks`, `All - Code Reviews`. A review or a document open in the tab names itself instead (`ATM - CR-134`, `ATM - sidebar-overhaul-design.md`) and keeps the item's icon. `/new` is *New Tab*.

**The change:**
- `places.ts`: `tabTitle(address, project)` gives the title and icon, and `scopeName` the scope:
  - the project's key;
  - else its name, cut to 16 characters with `…`;
  - else its ID, while the projects aren't listed;
  - All is `All`.
  The landing page (`/`) is All's Overview. A document's name drops a link's lines (`x.md#L4-L9` is `x.md`).
- `AppTitleBar.svelte`: each tab's label is its item's icon and the title, for the chosen tab and the others alike, and so is its entry in More ⌄. A review's tab shows the review's title on hover. Tabs no longer show the project's logo; the sidebar's scope does.
- **atm:** the project summary now has `key` (`meta.key`, null without one or for an empty one). Without it, a tab could show the key only once its project's tasks had loaded, and its title would flip from the name to the key. Added to `api.schema.json` and `multi-project-api.md`. The window's own title (the OS's) is unchanged: the page's title, as before.

**Tests:**
- `places.test.ts`: each item of a project (the Overview with and without its tab, Inbox, Tasks' three views, Code Reviews, Documents, Analysis' two) and a task open over a page. All's items and the landing page, in another workspace too. A review (and its sub-tab and round), a document (over a project's page and over All's, with a line link). A project without a key, a long name (16 characters), one not listed, a blank key. `/new`.
- `AppTitleBar.component.test.ts`: the chosen tab and another by their own addresses, with the key and the icons.
- `summary/tests.rs`: `key` for a keyed project, null for one without and for an empty one. `serve.test.mjs`: the summary's `key`.
- `app.spec.ts`, `keys.spec.ts`: the tabs' titles where they checked a project's name. The review tab is `Standard Project - CR-2`, its title on hover.
- `pnpm run check`, the unit suite, `cargo test -p atm-lib`, clippy, the CLI suite (serve), the functional e2e suite (586 passed).
