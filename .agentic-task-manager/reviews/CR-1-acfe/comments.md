## (the review) · round 1 · t1
- [x] **Hakan Serce** · 2026-11-18T00:47:20Z · c1
  your description is not complete. show the steps first before referring to them. What is 1 to 4?
  - **claude:0f56a916** · 2026-11-18T00:48:06Z · c2
    Fair: the summary named steps it never stated. The document now opens with them, in short, before anything refers to them (round 2): 1 skip drawing files off screen (CSS content-visibility); 2 give back far files' rows, keeping their height; 3 draw a long file in chunks; 4 cache a file at a revision in the browser; 5 highlight only what is shown, only if still slow; 6 page the files from atm and 7 speed up atm's answer, both as backlog tasks. Asked: build 1-4 now.
    - **Hakan Serce** · 2026-11-18T00:49:32Z · c3
      overview didn't change, though. I was expecting to see 1-4 in the Description of the CR.
      - **claude:0f56a916** · 2026-11-18T00:51:07Z · c4
        Right, the Description is what you read. atm had no way to change a review's summary, so I added one (`atm review round --summary-file`, with this round): the Description now lists the four steps asked for, each with what it does, then 5 to 7 and the budgets.

## docs/review-performance.md:67-72 · round 1 · t2
>
> ## What is decided, then built
>
> Build **1 to 4** in this task, with budgets in `reviews.spec.ts` for the long review: the first file drawn under
> 200 ms (twice in CI), a scroll step's slowest frame under 50 ms after the first screen, and the rows in the page
> bounded (no more than ~1,500 wherever the pane is); **5** if the app is still slow after them, measured on CR-1 in the

- [ ] **Hakan Serce** · 2026-11-18T00:54:52Z · c5
  I only see the proposal here, no code change. If they are coming after, then change title, if not add them to CR.
  - **claude:0f56a916** · 2026-11-18T00:55:19Z · c6
    They come in this review: I am building 1 to 4 now, and the next round will be the code with this document, the title kept. The round after the proposal comes with the numbers measured again against the budgets.
    - **Hakan Serce** · 2026-11-18T01:00:14Z · c7
      ok, I am keeping this open then, add the files again and set the state to review requested or whatever is the appropriate state after that, then.
