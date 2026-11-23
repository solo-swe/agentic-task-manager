+++
id = "01a2040e-e3b5-73fc-be90-04329d08d2ec"
number = 112
title = "The review page follows atm: can and editable"
status = "approved"
tasks = ["ATM-723"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-22T16:20:14Z
updated = 2026-11-22T17:27:22Z
+++

The review page shows its controls only as atm's `can` allows, in atm's words, and keeps no rule of its own.

## What changed
- **atm-review** `rules::may_edit`: one rule for who may change a comment's text (their own, not a poke, the review not ended). `Reviews::edit` uses it.
- **atm-lib** `review_pages/page.rs`: each comment and reply on the page carries `editable` from that rule.
- **Dashboard** `ReviewPage.svelte`: Approve, Comment, Request changes, Close and Poke follow `can`. Gone: the `ended` check, `settled`, the open-comments and drafts counting behind Approve, the `!sentBack` fallback. `can` is required in the page's type. `live` and `sentBack` stay for how the page looks and the button names only.
- **Dashboard** `ChainBox.svelte`: Edit shows by the comment's `editable`, not by comparing authors.
- **Tests**: a `may_edit` table over every status, a page test for `editable`, the page's unit tests given atm's answer per case, the e2e fixture answering `can` and `editable` as atm does, specs for a waiting, live, sent-back and approved review. The reviews list specs' dates are relative to the run now (they broke on 22 Nov, past the 5-day cutoff).

## To look at
- **Close** is still offered on a live review only, though atm allows its author or a reviewer to close any open review. Should a waiting review offer Close too?
- With Approve allowed by atm, the page shows the one-click Approve even if it holds comments it thinks are open: atm's answer wins.
