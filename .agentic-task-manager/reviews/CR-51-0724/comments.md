## tests/scripts/reviews.test.mjs:1195 · round 1 · t1
>       removeDir(root);

- [x] **Hakan Serce** · 2026-11-19T07:12:55Z · c1
  This file is huge. Can we split this file into multiple files under a reviews folder?
  - **claude:0f56a916** · 2026-11-19T07:25:01Z · c2
    Will do: I am splitting it into tests/scripts/reviews/, one file per area (the review lifecycle, refusals and exit codes, keys, submodules, listening), with the shared fixtures and helpers in one module there; run.mjs will find tests in that folder too. A new round follows.
  - **claude:0f56a916** · 2026-11-19T07:30:28Z · c3
    Done in round 2 (ba2bbdd, its own commit, so the round is that commit alone). tests/scripts/reviews/ now holds lifecycle.test.mjs (a review from opening to decision: rounds, comments, drafts, pokes), wrong-uses.test.mjs (refusals, exit codes, --dry-run; not refusals.test.mjs, which already exists one level up), keys.test.mjs, submodules.test.mjs and listening.test.mjs (wait, watch, catching up, the not-watching reminder), sharing fixtures.mjs. The split was mechanical, by line ranges, so no test text changed. run.mjs now also runs the *.test.mjs files one folder down (named by path or by name). The whole CLI suite passes: 829, the same tests as before plus nothing lost.
