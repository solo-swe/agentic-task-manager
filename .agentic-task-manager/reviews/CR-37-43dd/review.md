+++
id = "01a1f11d-5e0d-7273-9108-9515a72943dd"
number = 37
title = "Poke a review: a nudge its agent hears and answers"
status = "approved"
tasks = ["ATM-611"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T00:03:16Z
updated = 2026-11-19T06:01:41Z
+++

*Poke* on a review: your nudge, which its agent hears through its review watch and answers.

## Change
- **The model** (`atm-review`):
  - a poke is a comment on the review as a whole, on its latest round, marked `[poke]` in `comments.md` where a top-level comment has its box;
  - it is **never open**, so it never stands in the way of an approval;
  - an ended review (approved or closed) can't be poked;
  - pokes survive a merge like any comment.
- **One rule in every front end:** a person pokes, an agent answers and never pokes. This holds for:
  - `atm review poke CR-7 [--body '…']`;
  - `POST …/reviews/<CR>/pokes` `{body?}`;
  - the app's `poke_review`.
- **The watch:** each line now has `kind`, `poke` or `comment`.
- **What agents do** (`atm docs reviews`, the loop and do-task skills): on a poke, see where the review stands (comments not answered, a round owed, a question for you), do what's owed, and **reply to the poke** saying what they did or are waiting for.
- **The dashboard:**
  - *Poke* in the review page's top bar, with an optional message;
  - *Poke* on each row of the list, for reviews not decided yet (it shows *Poked* once sent, and the reason if refused);
  - in the conversation a poke carries a *Poke* mark, has no Resolve, and an empty one reads *Poked*.
- **Docs:** the route is in `docs/multi-project-api.md` and in `docs/security-architecture.md` I11.

## Before the release
This changes two skills (`atm-loop-tasks`, `atm-do-task`) and `docs/agent/`, so the written skill review in the security doc §3.3 has to cover it.

## Not here
*Poke* on the overview's Reviews tab (ATM-575's list of every project). It's easy to add once both are approved.

## Tests
- **Rust:**
  - the rules: a review-level poke, never open, approval not blocked, refused once approved;
  - `comments.md` round-trip with a reply under a poke;
  - the watch's `kind`;
  - the route table (with and without a message);
  - the app command (as the user).
- **CLI:** a person pokes (with and without a message); an agent is refused; `--open` lists none; the agent replies; approval goes through; poking after it is refused; dry run.
- **Dashboard:** the client call (REST and app), the row's *Poke* (decided reviews have none, sent, refused), the chain's mark, and an e2e from the page and the list.
- **Suites:** clippy, `cargo test`, the CLI suite and the unit tests pass. The functional e2e passes **except the same two timing tests under load**: switching tabs 204 ms, the long review's step 106 ms. They pass alone, and are noted on ATM-629.
