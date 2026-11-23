+++
id = "01a1fac8-edc9-7265-a555-2b062b7bb68e"
number = 94
title = "Review pages: line ranges, renames in git status, NUL-separated file lists, decisions all or none"
status = "approved"
tasks = ["ATM-670"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T21:07:14Z
updated = 2026-11-20T22:39:35Z
+++

Audit findings R4, H1, R14 and R7, all on the review pages.

**R4, a reversed line range.** A comment's lines must run forwards from line 1:
- `rules::comment` refuses `(5, 3)`, `(0, 2)` and `(0, 0)` for the CLI, REST and the app alike;
- a `comments.md` heading with such a range is kept unread, and `doctor` reports it;
- `follow` takes a reversed range by its two ends in order, instead of indexing an empty list, which panicked the page.

**H1, `git status -z` in the live review's look at the working copy.** A rename's old path is the record after it, with no status prefix, but three bytes were cut off it too: `README.md` read as `DME.md`, and a multibyte first character panicked. Records are now parsed as status and path, with a rename's or copy's old path as one more field, through `get(3..)`.

**R14, the changed-file lists of a diff.** They are read with `-z`, so a path with a tab or a quote comes back as it is, never C-quoted.
- *Not done:* a type change (`T`) still shows as modified. The model has no type-change status, and adding one would change the page and the file formats.

**R7, a decision split across tasks.** `submit` wrote the review's decision, then changed each task, so a failure on the second task left them split. Now `check_decision` runs every task in review through the state machine on a copy, with the decision's options, before anything is written. All or none, as `openings` does when a review is opened.
- *Worth a look:* this covers what the state machine refuses. A claim another agent holds is still only checked as each task changes, though a task in review normally has no claim.

**Tests**
- atm-review:
  - the comment rule's table of ranges;
  - `5-3` and `0` headings kept unread;
  - `follow` on `(12, 10)` and `(5, 3)`.
- atm-vcs:
  - a staged rename under `éé/`, both paths whole;
  - Unix only: a path with a tab and quotes, and a rename to `éé moved.md`, both listed as they are.
- atm-lib: `check_decision` names the task it can't reach and writes nothing; it leaves a task not in review alone and names an unknown task.
- CLI, `wrong-uses.test.mjs`: a staged rename of `éé/README.md` while a live review is open. The review reads, and the next checkpoint lists the rename by both paths.
- Passing: the crates' tests, the CLI suite and clippy.
