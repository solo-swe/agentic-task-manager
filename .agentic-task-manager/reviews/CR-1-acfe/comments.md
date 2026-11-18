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

- [x] **Hakan Serce** · 2026-11-18T00:54:52Z · c5
  I only see the proposal here, no code change. If they are coming after, then change title, if not add them to CR.
  - **claude:0f56a916** · 2026-11-18T00:55:19Z · c6
    They come in this review: I am building 1 to 4 now, and the next round will be the code with this document, the title kept. The round after the proposal comes with the numbers measured again against the budgets.
    - **Hakan Serce** · 2026-11-18T01:00:14Z · c7
      ok, I am keeping this open then, add the files again and set the state to review requested or whatever is the appropriate state after that, then.
  - **claude:0f56a916** · 2026-11-18T01:13:13Z · c8
    The code is in this review now: round 5 is the commit with steps 1 to 4 (the cards, the far files, the chunks, the cache), their tests, and this document with the numbers after them (*Built, and measured again*). The title stays. The Description says what changed and one budget that differs from what was asked: 100 ms a step, not 50.

## (the review) · round 5 · t3
- [x] **Hakan Serce** · 2026-11-18T01:32:15Z · c9
  the values you are showing in the measurements are small (83 ms) or didn't change too much from a user perspective (2.7 to 1.4 s). So, I am not sure if we are measuring the actual user experience correctly. Because 83ms seems fast enough to me, but reality definitely feels heavier, like 2 seconds or so.

  And scrolling to the bottom is a totally different story, because it actually cannot render for a while, I would say it is more like around 10 seconds mark than 2 in terms of how it feels, maybe during scrolling renderings keep queueing up or something.
  - **claude:0f56a916** · 2026-11-18T01:32:48Z · c10
    Agreed: the test misses what makes the app heavy. It draws plain text in Chromium from a mocked server. The app draws CompMD-highlighted sides (and renders the 394 KB spec), each fetched from atm, in WebKit (the app is a WKWebView, not Chromium). I am measuring CMD CR-1 itself next: the real review served by atm, in WebKit and Chromium, opening the Diffs tab and scrolling to the bottom, with where the time goes (highlighting, layout, requests). Then I fix what that shows and put the real numbers in this document, replacing the test-only ones.
    - **Hakan Serce** · 2026-11-18T01:33:41Z · c11
      that sounds better.
  - **claude:0f56a916** · 2026-11-18T03:14:48Z · c12
    Round 6 is that measurement and its fixes. The real CR-1 in WebKit blocked for 2 to 3.6 s on opening and drew its last file at 4.9 s. Now it blocks for 1.4 s (one frame, the first TypeScript highlight, which is CompMD s) and draws the last file at 1.4 to 3.0 s. Details in the Description and in the document under CR-1 itself.

## (the review) · round 6 · t4
- [x] **Hakan Serce** · 2026-11-18T03:30:57Z · c13
  what is the suggestion for creating the task for Shiki? What are we going to do?
  - **claude:0f56a916** · 2026-11-18T03:31:28Z · c14
    The proposal: a task in **CompMD's own project (CMD)**, since the highlighter is CompMD's (atm only shows what it draws). It would say:

    - **What:** in WebKit (Safari, and the atm app's WKWebView), the first block of a language on a page takes about **1.3 to 1.5 s** to highlight, blocking the page; Chromium takes a few ms. Measured on CMD CR-1 in the atm app's review page: the first TypeScript run, 662 characters, took 2.2 s; the rest a few ms each.
    - **Likely cause:** Shiki readying the TypeScript grammar's regular expressions with the Oniguruma (WASM) engine, which WebKit compiles slowly the first time. To confirm first, timing a page with only one TypeScript block in Safari.
    - **What CompMD could do**, in order of preference:
      1. Highlight in a **Web Worker**, so a slow grammar never blocks the page: the block shows plain and is coloured when ready (what atm now does per run, but inside CompMD, for every host).
      2. Use Shiki's **JavaScript regex engine** for the grammars it supports, which starts without compiling WASM; measure whether it is faster in WebKit.
      3. Ready the grammars a page needs **in idle time**, before the first block needs them.
    - **Done when:** the first TypeScript block in WebKit blocks the page for under 100 ms, measured the same way.

    On atm's side nothing changes: the turns already keep this to one frame. If you agree, I will add that task to CMD (with `/atm-create-tasks` there), the numbers and the way to reproduce it (`real-review.spec.ts`) in it.
    - **Hakan Serce** · 2026-11-18T03:36:10Z · c15
      makes sense, create the task and share task link here.
      - **claude:0f56a916** · 2026-11-18T03:42:48Z · c16
        Created: [CMD-249](/projects/0bd2033e/list?task=CMD-249), in CompMD s backlog (no sprint), P2: The first block of a language highlights without blocking the page in WebKit. It has the numbers, the likely cause, the three options in order, done when under 100 ms, and how to reproduce it. Its key is still provisional (~CMD-249) until it is committed in CompMD, which I left to you: compmd s tasks.json is changed, not committed.
