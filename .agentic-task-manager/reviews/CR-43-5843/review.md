+++
id = "01a1f153-716c-71a4-82f6-1a06c4735843"
number = 43
title = "Edit your own comment or reply"
status = "approved"
tasks = ["ATM-638"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T01:02:20Z
updated = 2026-11-19T06:10:00Z
+++

Edit your own comments and replies on a review's page.

## Change
- **`review-changes.ts`:** `editComment(project, review, comment, body)` uses the comment's `PATCH` route with `{body}`, or in the app `update_review_comment`. atm has had this since ATM-572 (`atm review edit`), the author's own only.
- **`ChainBox.svelte`:** *Edit* on each comment and reply you wrote (its author is the page's `you`). It reuses the in-place box drafts use:
  - *Save* or *Cancel*; Escape cancels and ⌘ Enter saves;
  - a refusal keeps the box, with why.
  - Others' comments and pokes have no *Edit*, and neither does anything on an ended review (approved or closed).
- **`ReviewPage` and `ReviewsTab`:** wired through a context (`setCommentEdits`, beside the document and task links), so it reaches every chain: the diffs, the conversation, the Comments tab.

## Not here
**"Edited" isn't shown.** atm's comments don't record an edit (no field in `comments.md` or the model), so the page can't say a comment was edited, or when. Showing it needs a field in atm-review first, and that's a task of its own if you want it.

## Tests
- **Unit:** `editComment`'s request (route, method, body, token) and the app's command.
- **Component (ChainBox):** *Edit* only on your own comment and reply, not others' or a poke; Save sends it trimmed; a refusal keeps the box with why; Escape and Cancel keep the old text; saved, the box closes; none on an ended review.
- **e2e:** you edit your reply in place (`PATCH /CR-2/comments/c9`); the agent's comment has no *Edit*.
- **Suites:** the reviews specs (57), the unit tests (1,770) and the checks pass.
