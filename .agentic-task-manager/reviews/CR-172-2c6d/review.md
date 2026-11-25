+++
id = "01a20f09-c7ad-7149-97e0-6fcb920b2c6d"
number = 172
title = "The hidden sidebar is a thin rail of icons; a hover shows it"
status = "approved"
tasks = ["ATM-771"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T19:30:28Z
updated = 2026-11-24T19:32:01Z
+++

The hidden sidebar is now a thin rail of icons, and pointing at it shows the sidebar (ATM-771, from Hakan on CR-148).

**The rail (`Sidebar.svelte`):**
- With the sidebar hidden, a 44 px rail stays at the window's left edge, under the title bar, so the title bar keeps the window's width and room for macOS's window buttons.
- It holds the scope's icon (the project's logo, or All's grid, named in its title), then the scope's items' icons in the sidebar's order. Each item has its badge on the icon's corner and its name as its label and title, with no text.
- An icon opens its item as a sidebar item does: a new tab, and ⌘-click in the background.
- The page leaves the rail's room: a new `--sidebar-rail` beside `--sidebar-offset` (`app.css` body padding). The title bar's left margin takes it back (`AppTitleBar.svelte`), so the bar spans over the rail. The other full-width bands start at the rail's edge.

**Pointing at it shows the sidebar:**
- The pointer resting on the rail for 250 ms, or the keyboard's focus in it, shows the whole sidebar over the page (`peeking`: a shadow, above the title bar). The page doesn't move and nothing is saved.
- It hides when the pointer and the focus have both left it, the scope menu is closed, or Escape is pressed.
- **The delay matters:** shown at once, the sidebar covered the rail, so an icon could never be clicked. A press on an icon cancels it, so a quick click opens that icon's item.
- A click's focus doesn't hold the sidebar open after the pointer leaves.
- Show Sidebar (its button, also in the shown sidebar's top strip) pins it, as before, and the rail goes.

**The scope list's stacking:** its Bits UI wrapper now sets `z-index: 70` itself, above the sidebar, the page's header and the sidebar shown from the rail. The narrowest-sidebar test (ATM-774) caught the list under the sidebar's items and the header at its corners.

**Not changed:** with the sidebar hidden, the page still shows its own row of item tabs (ATM-752), as before. Say if the rail should replace it.

**Tests:**
- `Sidebar.component.test.ts` (16):
  - the rail: its items, badges, labels, no text, and the room it takes;
  - All's icon;
  - the pointer showing the sidebar only after the delay, and leaving hiding it, unsaved;
  - keyboard focus, focus leaving and Escape;
  - a click's focus not holding it, and a quick click opening its item without showing it;
  - Show Sidebar removing the rail.
- `app.spec.ts`:
  - the rail's place and width under the title bar, the page beside it;
  - pointing at it shows the sidebar over the page, which doesn't move;
  - an item there opens a tab, and moving away hides it;
  - a rail icon's own click opens a tab;
  - Show Sidebar pins it.
- `pnpm run check`, the unit suite (2095 passed), the functional suite (625 passed, 1 failed). The failure is the timed "a click on a tab, and Next Tab round them all": its first click into the second project took 299 ms against 200. It fails the same way without this change (166, 139, 266 ms over three runs), so I've added a task for it rather than counting it here.
