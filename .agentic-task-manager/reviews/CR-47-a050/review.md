+++
id = "01a1f1ab-f7d2-700d-9cff-591a08b4a050"
number = 47
title = "From a review, another project opens in the same tab"
status = "approved"
tasks = ["ATM-581"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T02:39:01Z
updated = 2026-11-19T06:13:11Z
+++

From a review open in its tab, clicking another project in the sidebar makes the tab that project's page, as you chose (a).

## What was wrong
`withProject` kept the review in the address: from `/projects/abc/reviews/CR-2`, the sidebar went to `/projects/def/reviews/CR-2`, the *other* project's CR-2 (or a "no such review" page). It could look like a review showing in the wrong project.

## Change
`routes.ts` `withProject`: a review open in the address (`review`, `round`, `against`, `file`) is its project's, as a task's details already are. Opening another project drops them, so the other project opens at its Reviews list in the same tab. The same project keeps them.

This is the one function the sidebar and the title bar's project chooser go through, so both get the fix, in the app and the browser. ⌘-click still opens a new tab (unchanged).

## Tests
- **Unit:** from a review with its round, against and file, another project gives `/projects/def/reviews`; the overview gives `/projects`; the same project keeps it all.
- **e2e (`app.spec.ts`):** a review in its own tab, then the sidebar's other project. Still two tabs, the second selected and labelled by that project, its page shown with no review page, and the first tab unchanged. It failed before the fix (the tab stayed on the review).
- **Suites:** the app and tabs specs (37), the unit tests and the checks pass.
