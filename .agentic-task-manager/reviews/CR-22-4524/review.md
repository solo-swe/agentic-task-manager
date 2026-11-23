+++
id = "01a1f027-324f-7321-ad4d-ea35a0a04524"
number = 22
title = "A review atm can not read is listed as such, saying why"
status = "approved"
tasks = ["ATM-617"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T19:34:23Z
updated = 2026-11-18T19:38:30Z
+++

A review atm can't read is now listed as such, saying why, instead of being left out without a word. That was how you saw 7 tasks in review and 1 open review.

**Where it shows:**
- **`atm review list`:** after the other reviews, whatever the filter: `CR-13 (CR-13-3b8e): can't be read: … A newer atm may have written it.`
- **The REST list:** `unreadable: [{folder, reference, problem}]`, whatever the query. The **Reviews tab** shows each as a row of its own (*Can't be read* and why, with no link), after the others. Even alone, the list isn't "empty".
- **A task's details:** its review link says *can't be read* (why on hover) in place of the status, once the Reviews tab has listed the review so. The page asks the server for nothing more.
- **`atm doctor`:** as before, it names the folder and the reason.

**One change from the task:** `atm review list --json` keeps its array of the reviews it can read, and prints the unreadable ones on stderr (`atm: CR-13 (…): can't be read: …`). The task asked for an `unreadable` field in that JSON. But making the output an object would break every reader of the array, the agents' scripts and 11 of the CLI tests among them, the same kind of break as ATM-613's. The REST list could take a new field without breaking anything, so it has one.

**How:**
- `atm-review`'s `FileStore::unreadable()`: from the same read as `problems()`, by folder, with the reference taken from the folder's number.
- `review_pages::list_reviews` adds it to the list; the CLI prints it.
- The dashboard: `UnreadableReview`, `ReviewList`'s `unreadable` rows, and `known.svelte.ts`'s `learnUnreadable` and `knownUnreadable` for `ReviewLink`.
- Docs: `atm docs reviews` (what an agent does with one: tell the user, never edit its files to make it readable) and `multi-project-api.md`.

**Tests:**
- atm-review: a status a newer atm wrote and a broken header are both kept, with their folder, reference and why; the readable review is still listed.
- The CLI suite: `list` with one unreadable review beside a readable one, `--json` (the array, and stderr), and under a filter. The REST list gets it, filtered or not.
- Units: the list's row (no link, after the others, and alone it isn't an empty list), and the details' link (*can't be read*, and readable again).
- E2e: the Reviews tab's row.

`cargo fmt` also reformatted `files.rs`'s `status_of`, from ATM-613.
