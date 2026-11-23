## (the review) · round 1 · t1
- [x] **Hakan Serce** · 2026-11-19T08:23:22Z · c1
  can you walk me through an example here
  - **claude:0f56a916** · 2026-11-19T08:25:05Z · c2
    Sure, and walking it through turned up a case this round gets wrong. Both cases below.

    ## The example the change fixes
    1. On `dev/a` an agent works ATM-12 and opens CR-1 on it. `review.md` stores the key as it was then: `tasks = ["ATM-12"]`.
    2. Something gives the task a new number, ATM-14, with `ATM-12` as its alias, and **no other task holds ATM-12**. The CLI test plays this by editing `tasks.json`.
    3. **Before:** every check compared CR-1's `"ATM-12"` with the task's key now, `ATM-14`, as strings:
       - `review list --task ATM-14` didn't list CR-1;
       - the agent's watch (its work being `ATM-14`) never printed your comments on CR-1;
       - `atm task state ATM-14 done` didn't see CR-1's approval.
    4. **Now:** each key a review stores is resolved as `--task` resolves one (`task_reference`, ADR-007 §5). The order is the task holding that key now, else the task with it as an alias. Then tasks are compared, not strings. `ATM-12` resolves through the alias to ATM-14, and all three work. That's the test in `reviews/keys.test.mjs`.

    ## The case it gets wrong: a real clash
    This is ADR-009 §3, the usual way a number moves:
    1. `dev/a` makes ATM-12 (task A) and opens CR-1 on it: `tasks = ["ATM-12"]`.
    2. `dev/b` also made ATM-12 (task B) and reached main first.
    3. Merging `dev/a`: B keeps ATM-12, A becomes **ATM-14** with alias `ATM-12`. CR-1 still says `ATM-12`.
    4. Resolving `ATM-12` gives **B**: a key held now wins over an alias, by §5, as for every command. So:
       - CR-1 is listed under B's task;
       - your comments go to B's agent, and A's agent never hears them;
       - CR-1's approval would count for B.

    The key alone can't tell A from B. Since the clash is exactly when an alias exists, this round only fixes the rarer case. I'm not counting ATM-630 as done.

    ## How to fix it
    1. **(My pick) Rewrite the review files when the number moves.**
       - At the merge, atm already leaves a record ("ATM-12 is now ATM-14", from the moved task's UUID). The next atm command reads it to tell the user.
       - That same step would rewrite `ATM-12` to `ATM-14` in the reviews that came with the moved side: reviews naming `ATM-12` that the base branch doesn't have. B's reviews are on the base, so they're left alone.
       - `review.md` keeps plain keys and they stay right. Reviews from before this change, or written outside a merge, still resolve as now.
    2. **Store the task's UUID beside its key** in `review.md` (`tasks = ["ATM-12"]`, `task_ids = ["…"]`). This is exact, but goes against "no UUIDs in review.md".
    3. **Leave it**, and have atm warn when a review's key is both a task's key and another task's alias ("CR-1 names ATM-12, which may be ATM-14 since a merge"). Simple, but it puts the work on you.

    Shall I do 1, with a CLI test that plays the clash for real (two branches, a merge through the driver)?
    - **Hakan Serce** · 2026-11-19T08:27:08Z · c3
      1
      - **claude:0f56a916** · 2026-11-19T08:28:42Z · c4
        Going with 1: when a merge moves a task’s number, atm rewrites the old key to the new one in the reviews that came with the moved side, the ones naming the old key that the base branch doesn’t have. The test will play a real clash: two branches, both make ATM-12, the merge driver moves one. I’ll pick it up on ATM-630 right after ATM-641 and send it as round 2 here.
        - **claude:0f56a916** · 2026-11-19T08:37:41Z · c5
          One change to 1 before I build it, because 1 breaks in a case I hadn't counted.

          **Where 1 breaks:** two feature branches, neither on main, merging into each other. Then neither task is on the base branch, so the merge moves the newer one. "The reviews the base branch doesn't have" are both tasks' reviews, and the old key can't tell them apart.

          **What I'd do instead:** the same goal (a review keeps finding its own task), from evidence the review already has, with no files rewritten:
          - When a review's key is **ambiguous** (one task holds it now, another has it as an alias), atm reads `tasks.json` at the review's own commit.
          - That commit is round 1's: a range's head, or a working round's base. For a review of files, or of a submodule, it's the commit that added its `review.md`.
          - At that commit, one UUID had that number: that's the review's task.
          - Keys that aren't ambiguous resolve as now, with no git read. The ambiguous case is only after a clash, so it's rare and cheap.

          Why it's better than rewriting:
          - It's exact in every merge direction: into main, main into a branch, two branches.
          - It doesn't depend on when the next atm command runs after the merge.
          - No review files change behind anyone's back: no surprise diffs on a branch, nothing to commit.
          - A review that can't be read that way (its commit gone after a squash, say) falls back to today's rule, and atm says the key is ambiguous.

          The test plays a real clash: two branches each make ATM-12 with a review on it, merged through the driver. Each review then finds its own task (listing, the watch, the approval), whichever way the merge goes.

          I'm going ahead with this. Say if you'd rather have the rewrite.
          - **Hakan Serce** · 2026-11-19T08:45:20Z · c6
            go ahead and send the new round
            - **claude:0f56a916** · 2026-11-19T08:53:52Z · c7
              Sent as round 2: a648ce5 alone (a648ce5~1..a648ce5), since ATM-621 and ATM-641 landed between the rounds. The summary has what changed and how it is tested. Both clash tests fail with the disambiguation off.
