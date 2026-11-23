+++
id = "01a1f7e4-5d10-7415-b9b9-9dd191163544"
number = 69
title = "More ⌄ lists the tabs that do not fit"
status = "approved"
tasks = ["ATM-685"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T07:38:20Z
updated = 2026-11-20T09:56:21Z
+++

When the title bar's tabs don't all fit, a **More ⌄** button after the ‹ › arrows lists the tabs out of view. Choosing one shows it, scrolled into view.

## Change
- **`ui/TitleBarTabs.svelte`:**
  - A `name` prop: a tab's name in words. The strip stays generic: it draws labels from the app's snippet and doesn't know what a tab is called.
  - With a `name`, when the tabs overflow, it keeps the part of the strip in view and lists the tabs not wholly in it (every tab is `width` wide). The button shows only while some tab is out of view.
  - The menu is Bits UI's `DropdownMenu`, drawn over the page (a portal), so nothing of the bar clips it. It opens under the button, at its right edge.
  - Choosing a tab selects it, and the strip's existing rule brings the active tab into view.
  - **Kept:** the ‹ › arrows, the wheel and dragging, as they were. The button has a fixed width, as the arrows do (ATM-639: WebKit sizes the strip by its content's widths).
- **`shell/AppTitleBar.svelte`:** a tab's name moved into `tabName(tab)` (the project's name, a review's title, Projects Overview). Both the label and the menu use it.

## Found by the e2e test
At first the menu was drawn under the page's subtitle: its click went to the page. Bits UI's wrapper takes its `z-index` from the menu, which had none. It now has 50, as the bar's other menus (`.menu`).

## Tests
- **Component** (`TitleBarTabs.component.test.ts`): which tabs More counts when scrolled to the start, the middle and the end (3, 4, 3 of six); no More without a `name`, or while all fit.
- **E2e** (`app.spec.ts`, tagged `@webkit`): 12 tabs in a 1,000 px window.
  - More shows and the bar still fits.
  - Its menu lists the other project's tab first, and not the active one (in view).
  - Choosing it shows that project, its tab wholly in the strip.
  - Then the last tabs are the ones listed.
  - Passes 2 of 2 in Chromium and WebKit. The overflow test (ATM-605) also checks there's no More with two tabs.
- **Checks:** `pnpm run check`, the unit tests (1,815), the app and projects specs (58).
