+++
id = "01a20e4b-bf44-7454-919c-ca08c578bdca"
number = 161
title = "A project's Overview links into the Board and the List"
status = "approved"
tasks = ["ATM-756"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T16:02:54Z
updated = 2026-11-24T16:36:53Z
+++

A project's Overview now links into the Board and the List, in place in the tab (ATM-756, [the design §2.4](docs/sidebar-overhaul-design.md#24-a-projects-overview-links-into-the-board)). Nothing else on the Overview changes.

**The links (`shell/ProjectTab.svelte`, its Vocabulary):**
- **Each sprint it lists and the Backlog:** the Board on that one (`/projects/<id>/kanban?sprint=<name>`, `?sprint=backlog`). That covers the active sprint, the planned ones and the closed ones once shown. A plain link, so a click opens in place and ⌘-click opens a tab of its own.
- **The counts of To Do, In Review and Blocked** (each status of those kinds): the List of those tasks over every sprint, with the status chosen in the Filters and the search cleared (`dashboard.showStatus`). The Overview counts the whole project, so the List shows every sprint (`?sprint=all`) to match. The other statuses stay text.
- They look like the text around them, and a link on hover.

**One limit:** a task page's address holds only the Sprint filter, not a status. So a status count's ⌘-click, or a copied link, opens the List over every sprint without the status chosen. A plain click does choose it. A `?status=` in the address would close that gap. Want a task for it?

**The addresses** (`sprint-param.ts`: `sprintBoardAddress`, `statusListAddress`, from `tabs.ts`'s new `projectPage`): the project and its workspace, nothing open over the page (no `?task=` or `?doc=`).

**Tests:**
- `ProjectTab.component.test.ts`: each sprint and the backlog link to the Board on it, the closed ones too once shown. The counts of to do, review and blocked kinds link to the List over every sprint, and Done stays text. A plain click on a count chooses that status alone, clears the search and opens the List; ⌘-click is left to the link. The addresses keep the workspace and drop what is open over the page.
- `project.spec.ts`: from the Overview, Checkout opens the Board on it (its 2 cards) and Back returns. The backlog too. To Do opens the List of its one task with one filter chosen, and Back returns.
- `pnpm run check`, the unit suite (2029 passed), the functional e2e suite (616 passed, the timed project included).
