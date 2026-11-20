<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { ProjectDocument } from '../documents';
  import { loadCompmd, processText, topBlocks, type Block, type DrawnLine, type GutterRow } from '../compmd';
  import DocumentPage, { type PagePart } from '../documents/DocumentPage.svelte';
  import { CONTEXT, linesUnder, renderedParts, type RenderedPart } from './diff';
  import type { Chain, Picked, Side } from './types';

  // One side of a Markdown file in the diff viewer's rendered view (docs/code-review-design.md §3.7, ATM-574): the
  // document viewer's own page (ATM-645: `DocumentPage`, the same paper, width, height and margin numbers), on its
  // desk (`.doc-sheet`: the viewer's page variables and the document style picked in Settings), with the lines this
  // side changed marked on their text and the selected chain's lines too. A comment starts as on code (ATM-650): the
  // + of the line under the pointer, pressed (or its number), dragged over more lines or Shift-pressed; or text
  // selected, the lines under it. Either way `onpick` says which source lines, and where their text ends on screen.
  let {
    doc,
    side,
    changed,
    selected = null,
    picking = null,
    label,
    docStyle = null,
    chains = [],
    chainBox,
    composer,
    onpick,
  }: {
    /** This side's chains with lines (ATM-651): each shows under the block its last line is in, the page cut there */
    chains?: readonly Chain[];
    /** How a chain shows: the source view's own box, with reply, resolve and edit */
    chainBox?: Snippet<[Chain]>;
    /** The comment being written on this side's `picking` lines: where it will land, under their block */
    composer?: Snippet;
    doc: ProjectDocument;
    side: Side;
    /** The lines of this side the patch changed */
    changed: ReadonlySet<number>;
    /** The selected chain's lines on this side */
    selected?: [number, number] | null;
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
  /** Where each chain, and the comment being written, ends: its block is shown, and the page is cut after it, the
   * chain shown in the gap (ATM-651), as the source view shows a chain under its lines */
  const ends = $derived(
    new Set([...chains.flatMap((c) => (c.anchor.lines ? [c.anchor.lines[1]] : [])), ...(picking ? [picking[1]] : [])]),
  );
  const near = $derived.by((): ReadonlySet<number> => {
    const chain = selected ? Array.from({ length: selected[1] - selected[0] + 1 }, (_, i) => selected[0] + i) : [];
    return new Set([...changed, ...chain, ...ends]);
  });
  const parts = $derived(blocks ? renderedParts(blocks, near, doc.lines, opened, CONTEXT, ends) : null);
  /** What shows after a part: the chains ending in it, the latest-starting first as the source view orders them, and
   * the comment being written when its lines end there */
  const chainsAfter = (part: PagePart) =>
    chains
      .filter((c) => c.anchor.lines && c.anchor.lines[1] >= part.from && c.anchor.lines[1] <= part.to)
      .sort((x, y) => (y.anchor.lines?.[0] ?? 0) - (x.anchor.lines?.[0] ?? 0));
  const writingAfter = (part: PagePart) => !!picking && picking[1] >= part.from && picking[1] <= part.to;

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

  const within = (range: [number, number] | null, n: number) => !!range && n >= range[0] && n <= range[1];
  const inSelected = (n: number) => within(selected, n);
  /** The boxes to mark: changed lines (the first of each run starts a change: n / p go to it), the chain's, and the
   * lines picked for a comment */
  const marks = $derived(
    drawn
      .filter((l) => changed.has(l.line) || inSelected(l.line) || within(held, l.line))
      .flatMap((l) =>
        l.rects.map((rect, i) => ({
          rect,
          kind: within(held, l.line)
            ? 'picked'
            : inSelected(l.line)
              ? 'commented'
              : side === 'old'
                ? 'removed'
                : 'added',
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
</script>

<svelte:window onmouseup={letGo} />

<!-- svelte-ignore a11y_no_static_element_interactions (a selection, made with the mouse, offers a comment) -->
<div class="diff-pane diff-{side} doc-sheet" data-doc-style={docStyle} onmouseup={select}>
  <div class="diff-pane-label">{label}</div>
  {#if parts}
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
      {#snippet after(part: PagePart)}
        {#if chainBox}{#each chainsAfter(part) as chain (chain.id)}{@render chainBox(chain)}{/each}{/if}
        {#if composer && writingAfter(part)}{@render composer()}{/if}
      {/snippet}
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
</style>
