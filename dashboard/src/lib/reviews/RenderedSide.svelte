<script lang="ts">
  import CompMd from '../documents/CompMd.svelte';
  import type { ProjectDocument } from '../documents';
  import {
    gutterRows,
    loadCompmd,
    processText,
    topBlocks,
    type Block,
    type DrawnLine,
    type GutterRow,
  } from '../compmd';
  import { linesUnder, renderedParts, type RenderedPart } from './diff';
  import type { Picked, Side } from './types';

  // One side of a Markdown file in the diff viewer's rendered view (docs/code-review-design.md §3.7, ATM-574): the
  // document viewer's page on its desk (`.doc-sheet`: the viewer's page variables and the document style picked in
  // Settings, a page of a letter's proportions at the least), with its gutter in the page's margin (each source line
  // numbered beside the text CompMD drew from it, T-406: `gutterRows`, the viewer's `.doc-gutter-lines`), the lines
  // this side changed marked on their text, and the selected chain's lines too. Selecting text offers a comment on the
  // lines under it (`onpick`).
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
  const folded = $derived(!!parts?.some((p) => p.type === 'fold'));

  /** Each part's lines where CompMD drew them, in the page's coordinates, and its gutter: by the part's first line */
  let placed = $state.raw<Record<number, { drawn: DrawnLine[]; rows: GutterRow[] }>>({});
  const shownParts = $derived((parts ?? []).filter((p) => p.type === 'shown'));
  const drawn = $derived(shownParts.flatMap((p) => placed[p.from]?.drawn ?? []));
  const rows = $derived(shownParts.flatMap((p) => placed[p.from]?.rows ?? []));

  // As the document viewer places a Markdown document's lines: CompMD's boxes, moved into the page's coordinates; a
  // part's gutter numbers its own lines, in its own height
  function place(part: RenderedPart, lines: DrawnLine[], from: HTMLElement) {
    const el = pageEl;
    if (!el) return;
    const page = el.getBoundingClientRect();
    const box = from.getBoundingClientRect();
    const style = getComputedStyle(from);
    const dx = box.left - page.left + from.clientLeft + parseFloat(style.paddingLeft || '0');
    const dy = box.top - page.top + from.clientTop + parseFloat(style.paddingTop || '0');
    const round = (n: number) => Math.round(n * 10) / 10;
    const moved = lines.map((l) => ({
      line: l.line,
      rects: l.rects.map((r) => ({
        x: round(r.x + dx),
        y: round(r.y + dy),
        width: round(r.width),
        height: round(r.height),
      })),
    }));
    const whole = !folded && part.from === 1;
    const next = gutterRows(
      moved,
      part.to,
      whole
        ? { top: 0, bottom: el.clientHeight }
        : { top: box.top - page.top, bottom: box.bottom - page.top, first: part.from },
    );
    const was = placed[part.from];
    if (JSON.stringify(was) === JSON.stringify({ drawn: moved, rows: next })) return;
    placed = { ...placed, [part.from]: { drawn: moved, rows: next } };
  }

  function open(fold: RenderedPart) {
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
  const rowHas = (row: GutterRow, test: (n: number) => boolean) => {
    for (let n = row.from; n <= row.to; n++) if (test(n)) return true;
    return false;
  };

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
  <article class="doc-page" class:folded data-kind="markdown" bind:this={pageEl}>
    {#if rows.length}
      <div class="doc-gutter doc-gutter-lines" aria-hidden="true">
        {#each rows as row (row.from)}
          <div
            class:blank={!row.drawn}
            class:marked={rowHas(row, (n) => changed.has(n) || inSelected(n))}
            data-line={row.from}
            style:top="{row.top}px"
            style:height="{row.height}px"
          >
            {row.from === row.to ? row.from : `${row.from}–${row.to}`}
          </div>
        {/each}
      </div>
    {/if}
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
    {#each parts ?? [] as part (part.from)}
      {#if part.type === 'shown'}
        <div class="rendered-part" data-from={part.from} data-to={part.to}>
          <CompMd
            {text}
            path={doc.path}
            only={folded ? [part.from, part.to] : undefined}
            onlines={(lines, from) => place(part, lines, from)}
          />
        </div>
      {:else}
        {@const count = part.to - part.from + 1}
        <button class="rendered-fold" data-from={part.from} onclick={() => open(part)}>
          Show {count} unchanged {count === 1 ? 'line' : 'lines'}
        </button>
      {/if}
    {/each}
  </article>
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
  /* A page: at the least as tall as a letter page is for its width, and as tall as its document past that (ATM-592:
     `min-height: auto` makes the ratio a minimum, its content's height the rest; the viewer's `min-height` is a
     share of its own scroller, which a card hasn't); its gutter in its left margin */
  .diff-pane .doc-page {
    aspect-ratio: 8.5 / 11;
    min-height: auto;
    padding-left: max(52px, var(--space-10));
  }
  /* Folded (ATM-591): as tall as what it shows, not a letter page */
  .diff-pane .doc-page.folded {
    aspect-ratio: auto;
  }
  .rendered-fold {
    display: block;
    position: relative;
    z-index: 1;
    width: 100%;
    margin: var(--space-4) 0;
    padding: var(--space-2) var(--space-6);
    border: none;
    border-top: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
    background: color-mix(in srgb, var(--accent) 6%, transparent);
    color: var(--accent);
    font: inherit;
    font-family: var(--font-sans);
    font-size: var(--text-xs);
    text-align: left;
    cursor: pointer;
  }
  .diff-pane :global(.doc-gutter-lines) {
    left: var(--space-3);
    width: 34px;
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
