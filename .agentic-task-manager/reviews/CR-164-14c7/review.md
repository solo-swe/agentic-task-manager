+++
id = "01a20e66-afe5-74f8-a0cd-724d1c2b14c7"
number = 164
title = "Dependency graph placeholder lines on small nodes"
status = "approved"
tasks = ["ATM-735"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T16:32:20Z
updated = 2026-11-24T16:36:23Z
+++

Zoomed out too far for its text (under 50%, ATM-629), a dependency graph node now shows two wiggly lines where its two lines of text are, so it still reads as a card with content (ATM-735). The first line is the key's, shorter, in its type's colour. The second is the name's, longer and muted. Its hover title (the key and name) stays.

**How:**
- `graph/placeholder.ts`: `wiggle(x, y, width)` makes an evenly rolling line of half-waves that ends at its length. `PLACEHOLDER` holds the node's two lines.
- `DepGraph.svelte`: the two lines are drawn once, in the graph's `<defs>` as a `<symbol>`, and each unlabelled node places them with one `<use>`. With 500 nodes that is one small element each, not two paths each, which keeps ATM-629's saving. It is a `<symbol>`, not a `<g>`, so the tests' counts of `#dep-graph g` don't change.
- `app.css`: no fill, round ends. 4 and 3 units thick, so they show at the zooms where text is dropped. The type's colour comes from the node's `--node-color`, which reaches the `<use>`.

**Tests:**
- `placeholder.test.ts`: the wiggle's start and first half-wave, even half-waves ending at its length, a length that isn't a whole number of steps, a short one. The node's two lines are inside it, the key's above and shorter.
- `graph-zoom.spec.ts` (also tagged `@webkit`, the app's engine; passes in both): at 60% a node has its text and no lines. At 48% it has no text, and its lines are drawn inside its box, over half its width. Back at 60%, its text again.
- `performance.spec.ts` (500 tasks): fitted at 10%, every node has its lines; at 100% none has.
- `pnpm run check`, the unit suite, the functional e2e suite (618 passed). One unit test, `boundaries.test.ts` (it runs ESLint), timed out at 5 s once while the machine was loaded (the suite took 41 s instead of 12) and passes alone.
