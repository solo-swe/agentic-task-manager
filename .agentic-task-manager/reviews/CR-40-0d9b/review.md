+++
id = "01a1f148-4653-73fd-a3f6-edaecebf0d9b"
number = 40
title = "Drafts: comments and replies sent together with your decision"
status = "approved"
tasks = ["ATM-612"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T00:50:08Z
updated = 2026-11-19T06:09:26Z
+++

Drafts, as GitHub's *start a review*: comments and replies you write and send together with your decision.

## What you see
- **Every box that writes a comment or a reply** (on lines of a diff, a submodule entry, the review as a whole, a reply) offers *Add to my review*, *Comment now* and *Cancel*.
  - With no drafts yet, *Comment now* is the main button (⌘ Enter), as today.
  - Once you have drafts, *Add to my review* is.
- **Your drafts show where comments do:** in the diff on their lines, in the conversation, on the Comments tab. Each is marked *Draft*, with *Edit* (in place) and *Remove*, and has no *Resolve* or *Reply*.
- **Submit review counts them**, and its box says "N drafts will be sent with it".
  - *Comment* needs no summary when you have drafts.
  - On a review sent back, the top button reads *Publish comments*.
  - *Approve* is disabled while a drafted comment would be open once sent, and says so.

## How it works
- **Where drafts are kept:** in your atm state folder (`drafts/`, a file per project, review and person), never in the review's folder or the repository. No one else, no agent's watch and no commit sees them until they are sent.
- **Who drafts:** a person; an agent is refused and comments at once.
- **Where they are placed:** a draft comment is anchored on its round, and the review's page places it as it places comments (`placed`, `drifted`), so it moves with the code across rounds. On a live review, one about what's there now is anchored when it is sent.
- **Sending** (`review_requests::submit_decision`): every check of the decision first (`Reviews::check_submit`, split out of `submit`, with the drafted comments counted as open on approving), then the drafts in the order written, then the decision.
  - A comment with no words of its own only sends the drafts, and anyone may send them.
  - A draft that can't be sent stays, with those after it, and the error names it.
- **The CLI:** `atm review draft add|list|edit|remove`. `atm review submit` now goes through the same function, so it sends drafts too.
- **REST:** `GET|POST reviews/<CR>/drafts`, `PATCH|DELETE …/drafts/<d>`. DELETE is new among the change methods, with the same checks and an empty JSON body.
- **App commands:** `list_review_drafts`, `add_review_draft`, `update_review_draft`, `remove_review_draft`.
- **Docs:** the design doc §3.4, the API doc, security I11, `atm docs reviews` and `atm docs commands`.

## Also in this task
b072e1d: the single page's guardrail goes from 1 MiB to 1.5 MiB, as you decided. Drafts had taken the page to 1,054,578 bytes.

## Tests
- **atm-review:** `check_submit` refuses what `submit` would, counts the drafts, and writes nothing.
- **atm-lib** (9 tests):
  - kept outside the review, numbered and never reused;
  - only one person's;
  - an agent refused;
  - what a draft refuses;
  - on a live review;
  - edited and removed;
  - sent with each decision and gone after;
  - approval refused before any is sent;
  - a refused decision sends none;
  - one that can't be sent stays.
- **REST:** the route table, refusals, the GET as the user, DELETE's body. The real server (`serve.test.mjs`): kept in the state folder and not the repository, the page carrying them placed, *Publish comments*.
- **CLI:** add, list, edit, remove; another person sees none; an agent is refused; approval refused naming the draft; sent back with them; `--comment` publishes.
- **App:** each command's answer and refusals.
- **Dashboard:** `withDrafts` and the client calls (unit), ChainBox and ReviewPage (component), and an e2e from a diff's lines through edit, remove and *Comment*.
- **Suites:** clippy, `cargo test`, the CLI suite and the unit tests pass; the e2e passes but for the size guardrail now raised.
