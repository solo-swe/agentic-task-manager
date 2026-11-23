+++
id = "01a1edb2-5371-76ed-af56-f995bc6bed56"
number = 19
title = "The rendered Markdown view folds as the source view does"
status = "approved"
tasks = ["ATM-591"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T08:07:29Z
updated = 2026-11-18T08:18:44Z
+++

The Diffs tab's **Rendered** view now folds a Markdown file the way the Source view does.

**What shows:** the top-level blocks (a heading, a paragraph, a list, a table, a diagram) within 3 lines of a change are shown, whole and never cut. That's the same context git gives the source view. The unchanged blocks between are folded behind **Show n unchanged lines**, from the document's first line to its last; a click opens a fold, which joins what's shown around it. The selected chain's lines count as near, so a comment's text is never folded away. Each side folds on its own, as each is its own page already. The line numbers are the source's, each shown part numbering its own lines. A folded page is as tall as what it shows, not a letter page; opened whole, it's the page as before (ATM-592).

**How:**
- `compmd.ts`: `topBlocks(tree)`, a document's top-level blocks and their source lines (CompMD's `origin`, T-406; an end at a line's start is the line before, as a list takes its last newline), and `blocksIn(tree, [from, to])`, which keeps only the blocks reaching into a range, plus every definition, so a reference still finds what it names.
- `CompMd.svelte`: an `only` prop. It processes the whole text, so data, references and footnotes are the document's, and draws only those blocks.
- `diff.ts` `renderedParts(blocks, near, total, opened)`: the pure rule for what's shown and what's folded. Lines without a block (blank lines, a frontmatter) go with what they lie between. A document with no blocks known is shown whole.
- `RenderedSide.svelte`: learns the blocks once (`processText`), then renders a `CompMd` per shown part and a fold button between them. `gutterRows` gains `first`, so a part numbers from its own first line.

Tests:
- `compmd-processor.test.ts`: a real document's blocks and their lines, and a range keeping the blocks it reaches plus the definition a link needs.
- `diff.test.ts`: `renderedParts`, with a change mid-document, at the start and at the end, two changes far apart and close together, folds opened, no change, no blocks, and a long block kept whole.
- E2e: a 40-paragraph document with paragraph 20 changed shows paragraphs 19–21 numbered 37–41, with folds of 36 and 37 lines; opening the first shows from line 1. The ATM-574 and ATM-592 rendered tests now open their folds first.

One cost: each shown part has CompMD process the whole document, so with k parts the text is processed k + 1 times. That's fine for documents; say if you want it cached.
