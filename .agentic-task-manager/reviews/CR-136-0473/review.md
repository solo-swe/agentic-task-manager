+++
id = "01a20ab6-7874-774a-849c-8bcd5ebc0473"
number = 136
title = "Inbox and Documents designs"
status = "approved"
tasks = ["ATM-728"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-23T23:21:00Z
updated = 2026-11-23T23:43:00Z
+++

The two designs that follow from the sidebar overhaul (CR-135, approved). Once these are agreed, all three get broken down into tasks for this sprint.

**Inbox** (`docs/inbox-design.md`): items with three states (created, discarded with a reason, injected with links to its tasks), stored in `inbox.json` beside `tasks.json` and archived like tasks. Also the `atm inbox` commands, the routes (the first write routes besides reviews, with the same guards), the page with its quick-add box, and a `/atm-triage-inbox` skill that triages with you. Questions in §9.

**Documents** (`docs/documents-design.md`): a project's Markdown files, tracked and untracked but not ignored, under the viewer's rules. They show as a tree or by recent change, with titles and a search, and open in the viewer in place. One new read route. Questions in §6.
