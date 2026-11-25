+++
id = "01a20e55-563b-7251-87b2-d07dcf105542"
number = 162
title = "All reviews: a project filter"
status = "approved"
tasks = ["ATM-757"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T16:13:23Z
updated = 2026-11-24T16:37:00Z
+++

All reviews (All's Code Reviews, `/reviews`) gets a project filter (ATM-757, [the design §2.5](docs/sidebar-overhaul-design.md#25-all-projects-code-reviews)): *All projects*, or one or more projects. It lives in the list's address (`?projects=atm,compmd`), so a reload, Back and a link keep it. It sits beside the search in the title bar, as §6 has it.

**The change:**
- `overview/ReviewProjects.svelte`: a menu (Bits UI) with *All projects*, then a checkbox item per project. It stays open while you pick several. Its button says what is chosen: *All projects*, the one project's name, or *N projects*.
- `address.ts`:
  - `reviewProjectsOf` reads the projects named, in the workspace's order, leaving out unknown and empty ones; only unknown ones means every project.
  - `withReviewProjects` writes them; none means no parameter.
- `OverviewReviews.svelte`: the projects chosen come from the address, and a choice is a step in the tab's history. It replaces the single-project `<select>`, which took one project and wasn't in the address. The filter shows when the workspace has more than one project, as the picker did.
- **Beside the search:** the list's search offer (ATM-738's) can carry an `aside`, and `ReviewSearch` draws it next to the field. So in the app it's on the tab's line, and in the browser in the title bar.
- `multi-project-api.md`: `/reviews` with `?projects=`.

**Tests:**
- `address.test.ts`: read (none, empty, one, several in the workspace's order, unknown and blank entries left out, only unknown means every project, another workspace); written (several, none drops the parameter and keeps the rest); read back as written.
- `ReviewProjects.component.test.ts`: the button's text for all, one and several; the menu lists All projects then each project, ticked when chosen; a pick adds in the projects' order and a second pick takes it out; All projects clears the choice.
- `overview-reviews.spec.ts`: the filter in the title bar narrows to one project, the address says `?projects=second`, and a reload keeps it. Adding the second project gives 2 rows and *2 projects*, and Back goes to the one before. *All projects* clears the address, Back again. Then a row opens its review.
- `pnpm run check`, the unit suite (2038 passed), the functional e2e suite (616 passed).
