<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { ProjectDocument } from '../documents';
  import { loadCompmd, processText, topBlocks, type Block, type DrawnLine, type GutterRow } from '../compmd';
  import DocumentPage from '../documents/DocumentPage.svelte';
  import { CONTEXT, linesUnder, renderedParts, type RenderedPart } from './diff';
  import type { Chain, Picked, Side } from './types';

  // One side of a Markdown file in the diff viewer's rendered view (docs/code-review-design.md §3.7, ATM-574): the
  // document viewer's own page (ATM-645: `DocumentPage`, the same paper, width, height and margin numbers), on its
  // desk (`.doc-sheet`: the viewer's page variables and the document style picked in Settings), with the lines this
  // side changed marked on their text and the selected chain's lines too. A comment starts as on code (ATM-650): the
  // + of the line under the pointer, pressed (or its number), dragged over more lines or Shift-pressed; or text
  // selected, the lines under it. Either way `onpick` says which source lines, and where their text ends on screen.
  //
  // Its comments (ATM-651, Hakan: "like a callout from the line, like MS Word"): a column right of the page, each chain
  // in a balloon level with the text it is on, a line from that text to it; the comment being written too. The page is
  // never cut for them: it stays whole, as the document viewer draws it
  let {
    doc,
    side,
    changed,
    selected = null,
    selectedChain = null,
    picking = null,
    label,
    docStyle = null,
    whole = false,
    chains = [],
    chainBox,
    composer,
    onpick,
  }: {
    /** The whole document, nothing folded: the final view (ATM-731) */
    whole?: boolean;
    /** This side's chains with lines (ATM-651): each in a balloon in the margin, beside its text */
    chains?: readonly Chain[];
    /** How a chain shows: the source view's own box, with reply, resolve and edit */
    chainBox?: Snippet<[Chain]>;
    /** The comment being written on this side's `picking` lines: in a balloon beside them */
    composer?: Snippet;
    doc: ProjectDocument;
    side: Side;
    /** The lines of this side the patch changed */
    changed: ReadonlySet<number>;
    /** The selected chain's lines on this side */
    selected?: [number, number] | null;
    /** The selected chain: its callout drawn stronger */
    selectedChain?: string | null;
    /** The lines of this side whose comment is being written: their + stays, their text marked */
    picking?: [number, number] | null;
    label: string;
    docStyle?: string | null;
    /** Lines picked, and the bottom of their text on screen (a client y), where the comment box goes */
    onpick?: (picked: Picked, below: number) => void;
  } = $props();

  let pageEl = $state<HTMLElement>();

  // Folded as the source view is (ATM-591): the document's top-level blocks (CompMD's, from its source lines), those
  // near a change (or the selected chain) shown whole, the unchanged ones between them folded until opened. Each part
  // shown is the whole document processed, its blocks only drawn, so its references and data are the document's
  const text = $derived(doc.text ?? doc.document);
  let blocks = $state.raw<Block[] | null>(null);
  $effect(() => {
    const [source, path] = [text, doc.path];
    let gone = false;
    loadCompmd()
      .then((compmd) => {
        if (!gone) blocks = topBlocks(processText(compmd, source, undefined, path).tree);
      })
      // CompMD can't load: the page shows the text as written, whole
      .catch(() => {
        if (!gone) blocks = [];
      });
    return () => {
      gone = true;
    };
  });
  /** The folds opened, by their first line */
  let opened = $state.raw<ReadonlySet<number>>(new Set());
  /** Each chain's lines, and those of the comment being written: their blocks are shown, never folded, so each balloon
   * has its text beside it */
  const commented = $derived([
    ...chains.flatMap((c) => (c.anchor.lines ? [c.anchor.lines] : [])),
    ...(picking ? [picking] : []),
  ]);
  const range = (r: [number, number]) => Array.from({ length: r[1] - r[0] + 1 }, (_, i) => r[0] + i);
  const near = $derived.by((): ReadonlySet<number> => {
    return new Set([...changed, ...(selected ? range(selected) : []), ...commented.flatMap(range)]);
  });
  const parts = $derived(
    !blocks
      ? null
      : whole
        ? [{ type: 'shown' as const, from: 1, to: Math.max(doc.lines, 1) }]
        : renderedParts(blocks, near, doc.lines, opened, CONTEXT),
  );

  /** Where the page drew the lines, in its coordinates */
  let drawn = $state.raw<DrawnLine[]>([]);
  function placedOn({ drawn: lines }: { drawn: DrawnLine[]; rows: GutterRow[] }) {
    if (JSON.stringify(lines) !== JSON.stringify(drawn)) drawn = lines;
  }

  function unfold(fold: RenderedPart) {
    opened = new Set([...opened, fold.from]);
  }

  // Lines picked by their + (ATM-650): pressed on a row, the pick runs from it to the row under the pointer while the
  // button is held; let go, it is offered for a comment
  let dragged = $state<{ at: [number, number]; lines: [number, number] } | null>(null);
  function press(lines: [number, number], extend: boolean) {
    // Text selected before is not what this pick is about (letting go would offer it too)
    window.getSelection()?.removeAllRanges();
    const from = extend && picking ? picking : lines;
    dragged = { at: from, lines: [Math.min(from[0], lines[0]), Math.max(from[1], lines[1])] };
  }
  function over(lines: [number, number]) {
    const d = dragged;
    if (!d) return;
    const next: [number, number] = [Math.min(d.at[0], lines[0]), Math.max(d.at[1], lines[1])];
    if (next[0] !== d.lines[0] || next[1] !== d.lines[1]) dragged = { ...d, lines: next };
  }
  function letGo() {
    const d = dragged;
    if (!d) return;
    dragged = null;
    const el = pageEl;
    if (!el || !onpick) return;
    const [from, to] = d.lines;
    // The source lines as written, as the source view quotes them
    const quote = text
      .split('\n')
      .slice(from - 1, to)
      .join('\n');
    // Under the last line's text: its lowest box drawn, else its row in the margin
    const boxes = drawn.filter((l) => l.line >= from && l.line <= to).flatMap((l) => l.rects);
    const bottom = boxes.length ? Math.max(...boxes.map((r) => r.y + r.height)) : 0;
    onpick({ side, lines: [from, to], quote }, el.getBoundingClientRect().top + bottom);
  }
  /** The lines whose + stays: those being dragged over, else those whose comment is being written */
  const held = $derived(dragged?.lines ?? picking);

  const within = (r: [number, number] | null, n: number) => !!r && n >= r[0] && n <= r[1];
  const inSelected = (n: number) => within(selected, n);
  const inAnyChain = (n: number) => chains.some((c) => within(c.anchor.lines, n));
  /** The boxes to mark: changed lines (the first of each run starts a change: n / p go to it), the lines picked for a
   * comment, the selected chain's, and, faintly, every chain's (as Word marks the text a comment is on) */
  const marks = $derived(
    drawn
      .filter((l) => changed.has(l.line) || inSelected(l.line) || within(held, l.line) || inAnyChain(l.line))
      .flatMap((l) =>
        l.rects.map((rect, i) => ({
          rect,
          kind: within(held, l.line)
            ? 'picked'
            : inSelected(l.line)
              ? 'commented'
              : changed.has(l.line)
                ? side === 'old'
                  ? 'removed'
                  : 'added'
                : 'anchored',
          start: changed.has(l.line) && i === 0 && !changed.has(l.line - 1),
        })),
      ),
  );
  /** A margin's row is marked when one of its lines changed or is the chain's */
  function rowMarked(from: number, to: number) {
    for (let n = from; n <= to; n++) if (changed.has(n) || inSelected(n)) return true;
    return false;
  }

  function select() {
    const el = pageEl;
    const selection = window.getSelection();
    if (!el || !selection || selection.isCollapsed || !onpick) return;
    const box = el.getBoundingClientRect();
    const range = selection.getRangeAt(0);
    const rects = [...range.getClientRects()].map((r) => ({
      top: r.top - box.top,
      bottom: r.bottom - box.top,
    }));
    const lines = linesUnder(drawn, rects);
    if (lines) onpick({ side, lines, quote: selection.toString() }, range.getBoundingClientRect().bottom);
  }

  // ---- The margin (ATM-651) ----
  /** The comment being written, as a balloon of its own */
  const NEW = '\u0000new';
  /** The row the page and its margin share, and the margin itself: where the page sits in it, for the callouts */
  let rowEl = $state<HTMLElement>();
  let marginEl = $state<HTMLElement>();
  let geometry = $state<{ left: number; top: number; right: number; margin: number } | null>(null);
  function measureRow() {
    const page = pageEl;
    const margin = marginEl;
    const row = rowEl;
    if (!page || !margin || !row) return;
    const at = row.getBoundingClientRect();
    const p = page.getBoundingClientRect();
    const next = {
      left: p.left - at.left,
      top: p.top - at.top,
      right: p.right - at.left,
      margin: margin.getBoundingClientRect().left - at.left,
    };
    if (JSON.stringify(next) !== JSON.stringify(geometry)) geometry = next;
  }
  $effect(() => {
    const row = rowEl;
    if (!row || typeof ResizeObserver === 'undefined') return;
    // The row, the page and the margin: any of them resized moves where a callout starts or ends
    const sized = new ResizeObserver(() => measureRow());
    for (const el of [row, pageEl, marginEl]) if (el) sized.observe(el);
    measureRow();
    return () => sized.disconnect();
  });
  // The page drew its lines again (a fold opened, the window resized): where it sits may have moved too
  $effect(() => {
    void drawn;
    measureRow();
  });

  /** Where a range's text starts: the right end of its first visual line, and that line's middle, in the page's
   * coordinates; none when none of it is drawn */
  function anchorOf(lines: [number, number]): { x: number; y: number } | null {
    const rects = drawn.filter((l) => l.line >= lines[0] && l.line <= lines[1]).flatMap((l) => l.rects);
    if (!rects.length) return null;
    const top = Math.min(...rects.map((r) => r.y));
    const first = rects.filter((r) => r.y < top + 2);
    const height = Math.max(...first.map((r) => r.height));
    return { x: Math.max(...first.map((r) => r.x + r.width)), y: top + height / 2 };
  }
  /** Each balloon's height as drawn, by its id: the next one goes under it when they would overlap */
  let heights = $state<Record<string, number>>({});
  const GAP = 8;
  /** The balloons in the margin, top to bottom: each level with its text (its first line a little above the text's
   * middle), pushed down only past the one above it; the comment being written among them */
  const balloons = $derived.by(() => {
    if (!geometry) return [];
    const g = geometry;
    const entries = [
      ...chains.flatMap((c) => (c.anchor.lines ? [{ id: c.id, chain: c as Chain | null, lines: c.anchor.lines }] : [])),
      ...(picking && composer ? [{ id: NEW, chain: null, lines: picking }] : []),
    ]
      .map((e) => ({ ...e, anchor: anchorOf(e.lines) }))
      .filter((e): e is typeof e & { anchor: { x: number; y: number } } => e.anchor !== null)
      .sort((a, b) => a.anchor.y - b.anchor.y || (a.id === NEW ? 1 : b.id === NEW ? -1 : 0));
    let bottom = -Infinity;
    return entries.map((e) => {
      const y = g.top + e.anchor.y;
      const top = Math.max(y - 16, bottom + GAP);
      bottom = top + (heights[e.id] ?? 64);
      return { ...e, top, from: { x: g.left + e.anchor.x + 4, y } };
    });
  });
  /** The margin as tall as its last balloon, so the row grows for one below the page's end */
  const marginHeight = $derived(balloons.length ? Math.max(...balloons.map((b) => b.top + (heights[b.id] ?? 64))) : 0);
  const hasMargin = $derived(chains.length > 0 || (!!picking && !!composer));
</script>

<svelte:window onmouseup={letGo} />

<!-- svelte-ignore a11y_no_static_element_interactions (a selection, made with the mouse, offers a comment) -->
<div class="diff-pane diff-{side} doc-sheet" class:with-margin={hasMargin} data-doc-style={docStyle} onmouseup={select}>
  <div class="diff-pane-label">{label}</div>
  {#if parts}
    <div class="doc-margined" bind:this={rowEl}>
      <DocumentPage
        {text}
        path={doc.path}
        kind="markdown"
        lines={doc.lines}
        {parts}
        marked={rowMarked}
        onplaced={placedOn}
        onunfold={unfold}
        onpress={onpick ? press : undefined}
        onover={over}
        {held}
        bind:page={pageEl}
      >
        {#snippet overlay()}
          {#each marks as mark, i (i)}
            <div
              class="diff-block-mark diff-{mark.kind}"
              data-change={mark.start ? '' : undefined}
              style:left="{mark.rect.x - 4}px"
              style:top="{mark.rect.y}px"
              style:width="{mark.rect.width + 8}px"
              style:height="{mark.rect.height}px"
            ></div>
          {/each}
        {/snippet}
      </DocumentPage>
      {#if hasMargin}
        <!-- The comments, each in a balloon beside the text it is on, as Word's margin -->
        <div class="doc-margin" bind:this={marginEl} style:min-height="{marginHeight}px">
          {#each balloons as b (b.id)}
            <div
              class="doc-balloon"
              class:selected={b.chain !== null && b.chain.id === selectedChain}
              class:writing={b.id === NEW}
              data-balloon={b.chain?.id ?? 'new'}
              style:top="{b.top}px"
              bind:offsetHeight={heights[b.id]}
            >
              {#if b.chain}{@render chainBox?.(b.chain)}{:else}{@render composer?.()}{/if}
            </div>
          {/each}
        </div>
        <!-- Callouts: from the text, along to the page's edge, then to its balloon -->
        {#if geometry}
          <svg class="doc-callouts" aria-hidden="true">
            {#each balloons as b (b.id)}
              <polyline
                class:selected={b.chain !== null && b.chain.id === selectedChain}
                class:writing={b.id === NEW}
                points="{b.from.x},{b.from.y} {geometry.right},{b.from.y} {geometry.margin},{b.top + 16}"
              />
            {/each}
          </svg>
        {/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  /* Without a document style: a white page (the theme's background) on a grey desk */
  .doc-sheet:not([data-doc-style]) {
    --doc-desk: var(--surface2);
    --doc-page: var(--bg);
  }
  /* The desk the page lies on */
  .diff-pane {
    min-width: 0;
    padding: var(--space-6) var(--space-8) var(--space-9);
    background: var(--doc-desk);
  }
  .diff-pane-label {
    max-width: 820px;
    margin: 0 auto var(--space-4);
    color: var(--text-muted);
    font-size: var(--text-xs);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
  /* The page is the viewer's (ATM-645): as tall as its document, its width and margin the viewer's. Out of a scroller
     of its own, the viewer's "fill the window" height is nothing here */
  .diff-pane :global(.doc-page) {
    min-height: 0;
  }
  /* The page and its comments' margin, side by side (ATM-651); the callouts over both */
  .doc-margined {
    position: relative;
    display: flex;
    align-items: flex-start;
    gap: var(--space-8);
  }
  /* The page takes the room the margin leaves (up to its own width), never its content's least: its text is CompMD's,
     which a flex item sized by its content shrank to a letter a line */
  .doc-margined > :global(.doc-page) {
    flex: 1 1 0%;
    min-width: 0;
  }
  /* The margin's width, and the room it takes beside the page (the split gives a side with one that much more) */
  .diff-pane {
    --doc-margin-width: 280px;
    --doc-margin-room: calc(var(--doc-margin-width) + var(--space-8));
  }
  .doc-margin {
    position: relative;
    flex: 0 0 var(--doc-margin-width);
    width: var(--doc-margin-width);
  }
  .doc-balloon {
    position: absolute;
    left: 0;
    right: 0;
    z-index: 2;
    font-family: var(--font-sans);
  }
  /* A chain, or the comment being written, fitted to the balloon: no margin of its own (the callout ends at its edge),
     less padding, its text not indented under the author's avatar; the hint on a line of its own */
  .doc-balloon :global(.chain),
  .doc-balloon :global(.diff-composer) {
    margin: 0;
    padding: var(--space-4) var(--space-5);
    font-size: var(--text-sm);
  }
  .doc-balloon :global(.chain-body),
  .doc-balloon :global(.chain-reply),
  .doc-balloon :global(.chain-reply-button) {
    margin-left: 0;
  }
  .doc-balloon :global(.diff-composer-hint) {
    flex-basis: 100%;
    margin-right: 0;
  }
  .doc-callouts {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    overflow: visible;
    pointer-events: none;
    z-index: 1;
  }
  .doc-callouts polyline {
    fill: none;
    stroke: color-mix(in srgb, var(--yellow) 70%, var(--text-muted));
    stroke-width: 1;
    stroke-dasharray: 3 3;
  }
  .doc-callouts polyline.selected {
    stroke: var(--yellow);
    stroke-width: 1.5;
    stroke-dasharray: none;
  }
  .doc-callouts polyline.writing {
    stroke: var(--accent);
    stroke-dasharray: none;
  }
  .diff-block-mark {
    position: absolute;
    pointer-events: none;
    border-radius: var(--radius-sm);
  }
  .diff-removed {
    background: color-mix(in srgb, var(--red) 16%, transparent);
  }
  .diff-added {
    background: color-mix(in srgb, var(--green) 16%, transparent);
  }
  .diff-picked {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    box-shadow: inset 3px 0 0 var(--accent);
  }
  .diff-commented {
    background: color-mix(in srgb, var(--yellow) 24%, transparent);
    box-shadow: inset 3px 0 0 var(--yellow);
  }
  /* Text a comment is on, as Word marks it: faintly, until its chain is selected */
  .diff-anchored {
    background: color-mix(in srgb, var(--yellow) 12%, transparent);
  }
</style>
