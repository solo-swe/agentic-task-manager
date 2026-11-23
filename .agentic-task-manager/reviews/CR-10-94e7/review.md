+++
id = "01a1ed4b-5f05-7317-b541-98829c4094e7"
number = 10
title = "A review selected tab joins its page"
status = "approved"
tasks = ["ATM-608"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T06:15:02Z
updated = 2026-11-18T06:30:40Z
+++

The selected title-bar tab is drawn in the page's background (`--bg`) so it joins the page under it. A review's top bar was in `--surface`, so a review's tab looked unselected. That bar is now `--bg`, the same as a project's header. The review's own tabs (Overview, Diffs, Comments) are marked by their underline, so nothing else relied on the old colour.

Test: in `app.spec.ts`, with a review open in its own tab, the selected tab and the review's top bar have the same background, and the title bar a different one.

The review screenshots in the visual suite will change; they're regenerated in the release task.
