## docs/tabs-and-navigation-design.md:174 · round 1 · t1
> Every link in atm is a real link (<a href> with this address), so ⌘-click, a middle click, Copy Link and Open in 

- [x] **Hakan Serce** · 2026-11-20T11:31:35Z · c1
  what is the link when I am on the app? is it a atm:// something?
  - **claude:0f56a916** · 2026-11-20T11:32:41Z · c2
    Yes: `atm://` and the same address. The app already has deep links (T-459, `app/src-tauri/src/links.rs`): `atm://` and then the dashboard path and query, e.g. `atm://projects/<id>/reviews/CR-7/diffs?file=docs/x.md`. macOS hands them to the app, so the link opens the app at that page from anywhere (a chat, a terminal, a note). So in the app *Copy Link* copies the `atm://` form, and the browser copies its `http://localhost:…/` one. Both hold the same address, which `lib/address.ts` reads either way. One fix this turned up: deep links refuse a `#` fragment, so the comment link is `?comment=c12`, not `#c12`. §4.4 now says all this, and its table is fixed.

## docs/tabs-and-navigation-design.md:196-197 · round 2 · t2
> ⌘[ ⌘], mouse back and forward, a swipe	The browser's (the tab's)	The tab's history

- [x] **Hakan Serce** · 2026-11-20T11:33:59Z · c3
  We should also add back and forward arrows to the left of sidebar toggle button.
  - **claude:0f56a916** · 2026-11-20T11:34:28Z · c4
    Added. In the app, ‹ › arrows sit left of the sidebar toggle, after macOS window buttons, as Safari, Linear and Notion have them. They walk the active tab history: each is greyed out at its end, and a long press or a right-click lists the entries to jump to. With the sidebar open they move to its top strip, with the toggle. In the browser, the browser has its own arrows. §4.5 table, §5 (AppTitleBar, SidebarToggle) and §7 task 1 now say so.

## (the review) · round 3 · t3
- [x] **Hakan Serce** · 2026-11-20T11:35:50Z · c5
  Looks good, let's create a new sprint for this and create and add the tasks generated from this to the new sprint.
