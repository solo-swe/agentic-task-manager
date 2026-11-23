+++
id = "01a1f504-664d-74a9-8860-04a6080c9fd4"
number = 60
title = "Rendered Markdown as a document editor shows it: research"
status = "approved"
tasks = ["ATM-649"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T18:14:28Z
updated = 2026-11-20T09:50:40Z
+++

Why a review's rendered Markdown looks dark, and what it takes to show it as Word or Google Docs shows a document.

## In the doc
- **Editors (§2):** Word, Google Docs, Pages and Notion. Their pages, dark modes (sourced), type and margins. Word, Pages and the Docs apps keep the page light in a dark app, or turn it back with one click.
- **atm now (§3):** screenshots of the review's rendered side, dark and light. The cause:
  - in dark mode the page is painted in the theme's background, which is darker than its desk;
  - the default type is the task panel's (14 px text, 16 px titles);
  - every changed line is boxed in green or red.

  All of it is atm's CSS, nothing of CompMD's.
- **The gap (§4):** eight concrete changes, each with where it lives. One point to check with CompMD: highlighted code on a light page inside a dark app.
- **Recommendation (§5):** a *Document* page style as the default:
  - light paper in both modes, with a toggle as Word's Switch Modes;
  - reading type;
  - changes as bars in the margin;
  - comments in the right margin (ATM-651).
- **Follow-up tasks (§6)**, and **two questions for you (§7):** a light page by default in dark mode, and the line numbers in the viewer.

The three screenshots are in `docs/images/rendered-document/`.
