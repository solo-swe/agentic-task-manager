+++
id = "01a20eb3-6a56-77a1-bada-f5b2f72ab73e"
number = 168
title = "App sidebar: the scope chooser opens under its button"
status = "approved"
tasks = ["ATM-774"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T17:56:08Z
updated = 2026-11-24T18:41:53Z
+++

The sidebar's scope chooser (ATM-774). Round 2 answers Hakan's report: the list was squished in a narrow sidebar and clipped by it. It now shows on top of the sidebar's edge.

This round is one commit (`993f742`), on top of round 1's.

**Change (`Sidebar.svelte`):**
- The list is a floating Bits UI menu in a portal (`DropdownMenu.Portal` and `Content`, as the title bar's More tabs menu is), not static content inside the sidebar. It belongs to the window, so the sidebar's width and scroll no longer clip it, and it goes over the sidebar's edge onto the page when it's wider.
- It opens under the button, aligned with its left edge (`side="bottom"`, `align="start"`), kept 8 px inside the window.
- It is as wide as it needs: at least 240 px or the button's width, at most 360 px. It scrolls past 70% of the window's height.
- It takes `.menu`'s look (surface, border, radius, shadow) in its own rule. Round 1's `.menu-anchor` wrapper is gone.

**Tests (`app.spec.ts`):**
- `scopesPlaced`, used in the sidebar test and in a new test of the narrowest sidebar (180 px, a long project name, also run in WebKit). It checks that the open list:
  - is under its button and aligned with it, and whole in the window;
  - is outside the sidebar's DOM and on top at its corners;
  - is at least 240 px wide, with every name shown whole;
  - leaves the sidebar unscrolled.
- In the narrow case, the list goes past the sidebar's right edge, and a choice still works and closes it.

`pnpm run check`, the unit suite (2082 passed), the functional e2e suite (626 passed).
