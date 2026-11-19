<script lang="ts" module>
  /** What of a document a page shows: all of it, or (a review's rendered diff, ATM-591) its parts, each shown or folded */
  export interface PagePart {
    type: 'shown' | 'fold';
    from: number;
    to: number;
  }
</script>

<script lang="ts">
  import type { Snippet } from 'svelte';
  import { gutterRows, type DrawnLine, type GutterRow } from '../compmd';
  import type { DocumentRef } from '../documents';
  import LinePlus from '../ui/LinePlus.svelte';
  import CompMd from './CompMd.svelte';

  // A document's page (ATM-645): what the document viewer shows and a review's rendered Markdown shows, one component,
  // so both are the same page: the paper (`.doc-page`, the theme's and the document style's), the text as CompMD
  // renders it, and a Markdown document's line numbers in its margin, each beside the text CompMD drew from it (T-406:
  // `gutterRows`), the lines that draw nothing in the gaps. As tall as its document, as wide as the viewer's. Whoever
  // shows it adds its own marks over it (`overlay`: a link's lines in the viewer, a review's changes), says which rows
  // of the margin to mark (`marked`), and hears where the lines were drawn (`onplaced`). A review's diff shows the
  // document in parts (`parts`): the blocks near its changes, the others folded until opened (`onunfold`).
  let {
    text,
    path,
    kind,
    lines,
    parts,
    marked,
    localImage,
    onopen,
    onplaced,
    onunfold,
    onpress,
    onover,
    held = null,
    overlay,
    page = $bindable(),
  }: {
    /** A row of the margin pressed (its + or its number), as the source lines it numbers (ATM-650): the rows take a +
     * under the pointer only with it */
    onpress?: (lines: [number, number], extend: boolean) => void;
    /** The pointer over a row, as its lines */
    onover?: (lines: [number, number]) => void;
    /** The lines picked: their rows keep their + */
    held?: [number, number] | null;
    text: string;
    path: string;
    kind: string;
    /** How many lines the document has */
    lines: number;
    /** Its parts, shown or folded (none: the document whole) */
    parts?: readonly PagePart[] | null;
    /** Whether the margin's row for lines `from`–`to` is marked */
    marked?: (from: number, to: number) => boolean;
    localImage?: (url: string) => string | null;
    onopen?: (ref: DocumentRef) => void;
    /** Where the lines were drawn, in the page's coordinates, and the margin's rows, each time they move */
    onplaced?: (placed: { drawn: DrawnLine[]; rows: GutterRow[] }) => void;
    onunfold?: (part: PagePart) => void;
    /** Drawn over the page, under its text: the host's marks */
    overlay?: Snippet;
    /** The page's element */
    page?: HTMLElement;
  } = $props();

  const markdown = $derived(kind === 'markdown');
  const shown = $derived(parts ?? [{ type: 'shown' as const, from: 1, to: lines }]);
  const folded = $derived(shown.some((p) => p.type === 'fold'));

  /** Each part's lines where CompMD drew them, in the page's coordinates, and its rows: by the part's first line */
  let placed = $state.raw<Record<number, { drawn: DrawnLine[]; rows: GutterRow[] }>>({});
  const shownParts = $derived(shown.filter((p) => p.type === 'shown'));
  const drawn = $derived(shownParts.flatMap((p) => placed[p.from]?.drawn ?? []));
  const rows = $derived(shownParts.flatMap((p) => placed[p.from]?.rows ?? []));
  $effect(() => {
    onplaced?.({ drawn, rows });
  });
  // Another document: its own lines, once CompMD has drawn it
  $effect(() => {
    void path;
    placed = {};
  });

  // CompMD's boxes, moved into the page's coordinates; a part's margin numbers its own lines, in its own height (the
  // whole document: the page's)
  function place(part: PagePart, lines: DrawnLine[], from: HTMLElement) {
    const el = page;
    if (!el || !markdown) return;
    const box = el.getBoundingClientRect();
    const at = from.getBoundingClientRect();
    const style = getComputedStyle(from);
    const dx = at.left - box.left + from.clientLeft + parseFloat(style.paddingLeft || '0');
    const dy = at.top - box.top + from.clientTop + parseFloat(style.paddingTop || '0');
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
        : { top: at.top - box.top, bottom: at.bottom - box.top, first: part.from },
    ).map((r) => ({ ...r, top: round(r.top), height: round(r.height) }));
    const was = placed[part.from];
    if (JSON.stringify(was) === JSON.stringify({ drawn: moved, rows: next })) return;
    placed = { ...placed, [part.from]: { drawn: moved, rows: next } };
  }

  // Commenting (ATM-650), as the source view's gutter does: the row under the pointer (one that draws text) shows its
  // +; pressing it or its number picks its lines, Shift extends the pick; the host follows a drag with `onover`
  /** The row under the pointer, by its first line */
  let hover = $state<number | null>(null);
  function rowAt(e: MouseEvent): GutterRow | null {
    const el = page;
    if (!el) return null;
    const y = e.clientY - el.getBoundingClientRect().top;
    return rows.find((r) => r.drawn && y >= r.top && y < r.top + r.height) ?? null;
  }
  function move(e: MouseEvent) {
    if (!onpress) return;
    const row = rowAt(e);
    const at = row?.from ?? null;
    if (at !== hover) hover = at;
    if (row) onover?.([row.from, row.to]);
  }
  function press(e: MouseEvent, row: GutterRow) {
    if (e.button !== 0) return;
    // A drag picks lines, not text
    e.preventDefault();
    onpress?.([row.from, row.to], e.shiftKey);
  }
  const isHeld = (row: GutterRow) => !!held && row.from <= held[1] && row.to >= held[0];
</script>

<!-- A page on the desk: each theme may give it its own paper (--doc-page-*, themes/*.css) -->
<!-- svelte-ignore a11y_no_static_element_interactions (the pointer over a line shows its +; the + is the button) -->
<article
  class="doc-page"
  class:folded
  class:pickable={!!onpress}
  data-kind={kind}
  bind:this={page}
  onmousemove={move}
  onmouseleave={() => (hover = null)}
>
  {#if markdown && rows.length}
    <!-- A Markdown document's line numbers (T-406): each line beside its text, where CompMD drew it; lines that draw
         nothing in the gaps. In a review, each row that draws text takes a + under the pointer (ATM-650) -->
    <div class="doc-gutter doc-gutter-lines" id="doc-gutter" aria-hidden={onpress ? undefined : 'true'}>
      {#each rows as row (row.from)}
        <!-- svelte-ignore a11y_no_static_element_interactions (pressing a number picks its lines, as its + does) -->
        <div
          class:marked={marked?.(row.from, row.to) ?? false}
          class:blank={!row.drawn}
          class:picked={isHeld(row)}
          data-line={row.from}
          style:top="{row.top}px"
          style:height="{row.height}px"
          onmousedown={onpress && row.drawn ? (e) => press(e, row) : undefined}
        >
          {row.from === row.to ? row.from : `${row.from}–${row.to}`}
          {#if onpress && row.drawn && (hover === row.from || isHeld(row))}
            <LinePlus
              line={row.from === row.to ? row.from : `${row.from}–${row.to}`}
              held={isHeld(row)}
              left="calc(100% + var(--space-2))"
              onpress={(e) => press(e, row)}
            />
          {/if}
        </div>
      {/each}
    </div>
  {/if}
  {@render overlay?.()}
  {#key path}
    {#each shown as part (part.from)}
      {#if part.type === 'shown'}
        <div class="doc-part" data-from={part.from} data-to={part.to}>
          <CompMd
            {text}
            {path}
            {localImage}
            {onopen}
            only={folded ? [part.from, part.to] : undefined}
            onlines={(drawnLines, from) => place(part, drawnLines, from)}
          />
        </div>
      {:else}
        {@const count = part.to - part.from + 1}
        <button class="doc-fold" data-from={part.from} onclick={() => onunfold?.(part)}>
          Show {count} unchanged {count === 1 ? 'line' : 'lines'}
        </button>
      {/if}
    {/each}
  {/key}
</article>

<style>
  .doc-fold {
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
</style>
