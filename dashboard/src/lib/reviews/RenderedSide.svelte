<script lang="ts">
  import type { ProjectDocument } from '../documents';
  import { loadCompmd, processText, topBlocks, type Block, type DrawnLine, type GutterRow } from '../compmd';
  import DocumentPage from '../documents/DocumentPage.svelte';
  import { linesUnder, renderedParts, type RenderedPart } from './diff';
  import type { Picked, Side } from './types';

  // One side of a Markdown file in the diff viewer's rendered view (docs/code-review-design.md §3.7, ATM-574): the
  // document viewer's own page (ATM-645: `DocumentPage`, the same paper, width, height and margin numbers), on its
  // desk (`.doc-sheet`: the viewer's page variables and the document style picked in Settings), with the lines this
  // side changed marked on their text and the selected chain's lines too. Selecting text offers a comment on the lines
  // under it (`onpick`).
  let {
    doc,
    side,
    changed,
    selected = null,
    label,
    docStyle = null,
    onpick,
  }: {
    doc: ProjectDocument;
    side: Side;
    /** The lines of this side the patch changed */
    changed: ReadonlySet<number>;
    /** The selected chain's lines on this side */
    selected?: [number, number] | null;
    label: string;
    docStyle?: string | null;
    onpick?: (picked: Picked) => void;
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
  const near = $derived.by((): ReadonlySet<number> => {
    const chain = selected ? Array.from({ length: selected[1] - selected[0] + 1 }, (_, i) => selected[0] + i) : [];
    return new Set([...changed, ...chain]);
  });
  const parts = $derived(blocks ? renderedParts(blocks, near, doc.lines, opened) : null);

  /** Where the page drew the lines, in its coordinates */
  let drawn = $state.raw<DrawnLine[]>([]);
  function placedOn({ drawn: lines }: { drawn: DrawnLine[]; rows: GutterRow[] }) {
    if (JSON.stringify(lines) !== JSON.stringify(drawn)) drawn = lines;
  }

  function unfold(fold: RenderedPart) {
    opened = new Set([...opened, fold.from]);
  }

  const inSelected = (n: number) => !!selected && n >= selected[0] && n <= selected[1];
  /** The boxes to mark: changed lines (the first of each run starts a change: n / p go to it), and the chain's */
  const marks = $derived(
    drawn
      .filter((l) => changed.has(l.line) || inSelected(l.line))
      .flatMap((l) =>
        l.rects.map((rect, i) => ({
          rect,
          kind: inSelected(l.line) ? 'commented' : side === 'old' ? 'removed' : 'added',
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
    const rects = [...selection.getRangeAt(0).getClientRects()].map((r) => ({
      top: r.top - box.top,
      bottom: r.bottom - box.top,
    }));
    const lines = linesUnder(drawn, rects);
    if (lines) onpick({ side, lines, quote: selection.toString() });
  }
</script>

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
  .diff-commented {
    background: color-mix(in srgb, var(--yellow) 24%, transparent);
    box-shadow: inset 3px 0 0 var(--yellow);
  }
</style>
