+++
id = "01a1ed47-ccda-7311-b75b-261f5d01fde3"
number = 9
title = "Title bar tabs scroll in a strip of their own"
status = "approved"
tasks = ["ATM-605"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T06:11:08Z
updated = 2026-11-18T06:19:14Z
+++

More tabs than fit in the title bar used to widen the whole bar past the window. That pushed the window buttons out, and the page scrolled sideways with it. Now:

- The tabs scroll in a strip of their own. The menus, the bar's end and the window buttons keep their room; only the strip gives way.
- A wheel or trackpad over the strip scrolls it sideways (up and down too) and nothing else. A pinch is still the page's zoom.
- **‹ ›** show at the strip's ends only when the tabs overflow. Each scrolls by about one tab and is disabled at its end. The + stays right after the strip.
- The active tab is kept in view: when it changes, when tabs come or go, and when the strip's room changes. A scroll of yours stays put.

Tests:
- e2e (`app.spec.ts`), 12 tabs in a 1000 px window: the bar fits the window, the window buttons are in view, and the page doesn't scroll. The last (active) tab is shown, with only ‹ enabled. The wheel scrolls the strip and leaves the page alone. ‹ and › work to each end. With two tabs, there are no arrows.
- Unit (`TitleBarTabs.component.test.ts`): the wheel turns sideways and is never passed on, a pinch is left alone, and the arrows appear on overflow.
