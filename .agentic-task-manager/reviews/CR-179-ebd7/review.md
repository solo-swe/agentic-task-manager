+++
id = "01a20fd4-8cb4-76f7-97bc-33917879ebd7"
number = 179
title = "App: a switch to another project's tab loads its data first"
status = "approved"
tasks = ["ATM-777"]
author = "claude:33cba067"
reviewers = []
created = 2026-11-24T23:11:57Z
updated = 2026-11-24T23:22:43Z
+++

The timed test's first click into another project's tab took 106 to 173 ms alone (and up to 329 in the full suite) against 200, every other switch 30 to 100.

**Cause:** a switch to a project not shown before went through `reload()`, which waited for `list_projects` before it even asked for the project's data. Meanwhile the tab said *Second Project - Tasks* and the board still showed the **last** project's cards. Shown by a new test that holds the list back: the board stayed the first project's 11 cards.

**Fix** (`state.svelte.ts`): ATM-776's start path, for every load. `showKnownProjects` decides the page from the projects the page already knows (listed already, else at the app's first load the ones kept at its last start), the project's data is asked first and shown as soon as it is in, and the fresh list after it, which still corrects the page if it changed. A list out of date (a project gone) only means the project wasn't shown early: the usual path loads it and reports why it can't.

**Measured** (the timed test alone, `--repeat-each 5`): the first switch to the second project 94 to 96 ms (was 106 to 173), in the full suite 95.

**Tests:** `app.spec.ts`: a switch with the project list held shows the other project's own board; the timed test five times alone and in the full suite; the unit tests.
