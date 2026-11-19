<script lang="ts" module>
  /** A line of a run: its text, its number on each side shown (none: a padding line, where the other side has one),
   * and what changed */
  export interface CodeLine {
    text: string;
    old: number | null;
    new: number | null;
    kind: 'context' | 'removed' | 'added' | 'changed' | 'pad';
  }
</script>

<script lang="ts">
  import CompMd from '../documents/CompMd.svelte';
  import LinePlus from '../ui/LinePlus.svelte';
  import { fenced } from './diff';

  // A run of a file's lines in the diff viewer (docs/code-review-design.md §3.7, ATM-573): one fenced block of the
  // file's language, rendered by CompMD as the document viewer renders a source file, with the viewer's own gutter
  // (the line numbers, and a + on the line under the pointer) and the change marks beside it, placed where CompMD drew
  // the lines (their top and line height, measured after each render and on resize). Pressing a line's + or number
  // picks it (Shift extends the pick); dragging on extends it to the line under the pointer (`onpick`, `onover`); the
  // lines picked keep their + while the comment is written.
  let {
    lines,
    language,
    numbers = 'new',
    keys = [],
    changes = [],
    picked = null,
    marked = null,
    wrap = false,
    onpick,
    onover,
  }: {
    /** Long lines wrap at the column's width (ATM-588): each line's row as tall as CompMD drew it, its number on its
     * first visual line */
    wrap?: boolean;
    /** The lines of the chain selected, by their index in `lines`: marked */
    marked?: [number, number] | null;
    lines: CodeLine[];
    language: string | null;
    /** Each line's key in the viewer (its block and row), so the viewer's n / p can find it */
    keys?: string[];
    /** The lines that start a change, by index: n / p and the page's buttons go to them */
    changes?: number[];
    /** Which numbers the gutter shows: one side's, or both (the unified view) */
    numbers?: 'old' | 'new' | 'both';
    /** The lines picked, by their index in `lines` */
    picked?: [number, number] | null;
    /** A line pressed (its + or its number): picked, or the pick extended to it with Shift */
    onpick?: (index: number, extend: boolean) => void;
    /** The pointer over a line, while a pick is dragged */
    onover?: (index: number) => void;
  } = $props();

  const source = $derived(lines.map((l) => l.text).join('\n'));
  /** Nothing in it to highlight: padding rows and empty lines only */
  const blank = $derived(lines.every((l) => l.kind === 'pad' || l.text.trim() === ''));
  const text = $derived(
    fenced(
      lines.map((l) => l.text),
      language,
    ),
  );

  let el = $state<HTMLElement>();
  let layout = $state<{ top: number; lineHeight: number } | null>(null);
  /** The line under the pointer: its + shows */
  let hover = $state<number | null>(null);

  /** A line's height when CompMD couldn't draw the code (the text shows as written): the gutter still numbers it */
  const FALLBACK_LINE = 20;

  function measure() {
    if (!el) return;
    const code = el.querySelector<HTMLElement>('.compmd-page pre > code');
    if (!code) {
      const state = el.querySelector('.compmd')?.getAttribute('data-compmd');
      if (state === 'failed' && layout?.lineHeight !== FALLBACK_LINE) layout = { top: 0, lineHeight: FALLBACK_LINE };
      return;
    }
    // A line's height as drawn: the code's height over its lines (every one is drawn, an empty last one too: ATM-589),
    // so the rows never drift from the text; else what its style says. Its style only then (ATM-644): a computed
    // style read forces a restyle of the whole page, which takes a second in the app's WebKit on a long review
    const drawnHeight = code.getBoundingClientRect().height;
    const styled = () => {
      const style = getComputedStyle(code);
      return parseFloat(style.lineHeight) || parseFloat(style.fontSize) * 1.6 || 20;
    };
    const lineHeight = !wrap && lines.length && drawnHeight > 0 ? drawnHeight / lines.length : styled();
    const top = code.getBoundingClientRect().top - el.getBoundingClientRect().top;
    if (layout?.top !== top || layout.lineHeight !== lineHeight) layout = { top, lineHeight };
    if (wrap) measureWrapped(code);
  }
  // Wrapped or not: measured again (the code's lines take their new heights)
  $effect(() => {
    void wrap;
    requestAnimationFrame(() => measure());
  });

  $effect(() => {
    const target = el;
    if (!target) return;
    measure();
    const resize = new ResizeObserver(measure);
    resize.observe(target);
    // What CompMD draws, only (ATM-644): the run's own gutter (the + under the pointer, a line picked) changes nothing
    // CompMD drew, so it never measures the run again; every hover did, a forced restyle of the page each
    const drawn = target.querySelector('.compmd') ?? target;
    const rendered = new MutationObserver(measure);
    rendered.observe(drawn, { childList: true, subtree: true, attributes: true, attributeFilter: ['data-compmd'] });
    return () => {
      resize.disconnect();
      rendered.disconnect();
    };
  });

  /** Wrapped (ATM-588): each line's row in the run's pixels, from its first visual line to the next line's. Lines aren't
   * one height then, so the gutter follows the text. CompMD draws a code block's lines as text between newlines (and
   * says where only a Markdown document's lines are), so each line's top is read here, from a collapsed range at its
   * start: read only, nothing of CompMD's page is changed */
  let wrapped = $state.raw<{ top: number; height: number }[] | null>(null);
  function measureWrapped(code: HTMLElement) {
    if (!el) return;
    const origin = el.getBoundingClientRect().top;
    const starts: { node: Text; offset: number }[] = [];
    const walker = document.createTreeWalker(code, NodeFilter.SHOW_TEXT);
    let atStart = true;
    for (let node = walker.nextNode() as Text | null; node; node = walker.nextNode() as Text | null) {
      const text = node.data;
      for (let at = 0; at <= text.length; at++) {
        if (atStart && at < text.length) {
          starts.push({ node, offset: at });
          atStart = false;
        }
        if (text[at] === '\n') atStart = true;
      }
    }
    const range = document.createRange();
    // Where nothing is laid out (a test's page), there is nothing to measure: the lines keep one height
    if (typeof range.getClientRects !== 'function') return;
    const lineHeight = layout?.lineHeight ?? FALLBACK_LINE;
    // A character's box is its glyphs' (the font's height), centred in its line's box: the line starts half a line
    // height above that centre (ATM-652: taking the character's top put every row half the leading low)
    const tops = starts.map(({ node, offset }) => {
      range.setStart(node, offset);
      range.collapse(true);
      const rect = range.getClientRects()[0] ?? range.getBoundingClientRect();
      return (rect.top + rect.bottom) / 2 - lineHeight / 2 - origin;
    });
    const bottom = code.getBoundingClientRect().bottom - origin;
    // Empty lines at the end have no character to measure: one line height each, up from the code's end
    const missing = lines.length - tops.length;
    for (let k = missing; k > 0; k--) tops.push(bottom - k * lineHeight);
    const rows = lines.map((_, i) => {
      const top = tops[i];
      const next = i + 1 < lines.length ? tops[i + 1] : bottom;
      return { top, height: Math.max(next - top, lineHeight) };
    });
    if (JSON.stringify(rows) !== JSON.stringify(wrapped)) wrapped = rows;
  }
  $effect(() => {
    if (!wrap) wrapped = null;
  });
  /** Each line's row: measured when wrapped, else the code's one line height */
  const rowOf = (i: number) =>
    wrap && wrapped?.[i]
      ? wrapped[i]
      : layout
        ? { top: layout.top + i * layout.lineHeight, height: layout.lineHeight }
        : null;

  /** The line at a pointer's height in the run, if there is one there */
  function lineAt(e: MouseEvent): number | null {
    if (!el || !layout) return null;
    const y = e.clientY - el.getBoundingClientRect().top;
    let i: number;
    if (wrap && wrapped) i = wrapped.findIndex((r) => y >= r.top && y < r.top + r.height);
    else i = Math.floor((y - layout.top) / layout.lineHeight);
    return i >= 0 && i < lines.length && lines[i].kind !== 'pad' ? i : null;
  }

  function move(e: MouseEvent) {
    const i = lineAt(e);
    if (i !== hover) hover = i;
    if (i !== null) onover?.(i);
  }

  function press(e: MouseEvent, i: number) {
    if (e.button !== 0) return;
    // A drag picks lines, not text
    e.preventDefault();
    onpick?.(i, e.shiftKey);
  }

  const isPicked = (i: number) => !!picked && i >= picked[0] && i <= picked[1];
  const label = (l: CodeLine) =>
    numbers === 'both' ? [l.old ?? '', l.new ?? ''] : [(numbers === 'old' ? l.old : l.new) ?? ''];
  const starts = $derived(new Set(changes));
</script>

<!-- svelte-ignore a11y_no_static_element_interactions (the pointer over a line shows its +; the + is the button) -->
<div
  class="diff-code"
  class:wrap
  class:both={numbers === 'both'}
  class:pickable={!!onpick}
  bind:this={el}
  data-lines={lines.length}
  onmousemove={move}
  onmouseleave={() => (hover = null)}
>
  {#if layout}
    {#each lines as line, i (i)}
      {@const row = rowOf(i)!}
      <div
        class="diff-line diff-{line.kind}"
        class:picked={isPicked(i)}
        class:commented={!!marked && i >= marked[0] && i <= marked[1]}
        data-row={keys[i]}
        data-change={starts.has(i) ? '' : undefined}
        style:top="{row.top}px"
        style:height="{row.height}px"
        style:--line-height="{layout.lineHeight}px"
      >
        {#if line.kind !== 'pad'}
          <button
            class="diff-number"
            tabindex="-1"
            aria-label="Line {line.new ?? line.old}"
            onmousedown={(e) => press(e, i)}
            >{#each label(line) as n, k (k)}<span>{n}</span>{/each}</button
          >
          {#if onpick && (hover === i || isPicked(i))}
            <LinePlus
              line={line.new ?? line.old ?? ''}
              held={isPicked(i)}
              left="calc(var(--gutter) + var(--space-1))"
              top={wrap ? 'calc(var(--line-height) / 2)' : '50%'}
              onpress={(e) => press(e, i)}
            />
          {/if}
        {/if}
      </div>
    {/each}
  {/if}
  {#if blank}
    <!-- Nothing to highlight (a side of added or removed lines only: its padding rows, ATM-646): no CompMD page, whose
         opening and closing each restyle the whole page; the text as written, as a run waits for its turn below -->
    <div class="compmd" data-compmd="ready">
      <div class="compmd-page compmd-waiting"><pre><code>{source}</code></pre></div>
    </div>
  {:else}
    <!-- Plain until its turn: the page's runs are highlighted one at a time, nearest the view first (ATM-587) -->
    <CompMd {text} plain={source} />
  {/if}
</div>

<style>
  .diff-code {
    --gutter: var(--space-10);
    --plus: var(--space-8);
    position: relative;
    padding-left: calc(var(--gutter) + var(--plus));
    /* Nothing of a run is drawn past it (ATM-590): a scroll bar the system lays over the content (macOS's), or a row
       of its gutter, stays inside the run, off the fold row after it */
    overflow: clip;
    overflow-clip-margin: 2px;
    /* Never the pane's scroll anchor (ATM-655): a run's text is replaced when its turn to be highlighted comes, and
       WebKit, anchored on that text, scrolled the pane by the whole run's height (1,600 px after a comment box opened) */
    overflow-anchor: none;
  }
  .diff-code.both {
    --gutter: calc(var(--space-10) * 2);
  }
  .diff-code :global(.compmd-page pre) {
    margin: 0;
    white-space: pre;
    overflow-x: auto;
    background: transparent;
    /* Room under the last line for a scroll bar laid over the content (ATM-590), so it covers neither that line nor
       what comes after the run; a classic scroll bar takes its own room below it */
    padding-bottom: var(--space-4);
  }
  .diff-code :global(.compmd-page pre > code) {
    display: block;
  }
  /* Wrapped (ATM-588): long lines break at the column's width, anywhere if they must; no sideways scroll */
  .diff-code.wrap :global(.compmd-page pre),
  .diff-code.wrap :global(.compmd-plain) {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    overflow-x: hidden;
  }
  .diff-line {
    position: absolute;
    left: 0;
    right: 0;
    pointer-events: none;
  }
  .diff-removed {
    background: color-mix(in srgb, var(--red) 16%, transparent);
  }
  .diff-added {
    background: color-mix(in srgb, var(--green) 16%, transparent);
  }
  .diff-changed {
    background: color-mix(in srgb, var(--yellow) 14%, transparent);
  }
  .diff-pad {
    background: repeating-linear-gradient(
      135deg,
      transparent 0 4px,
      color-mix(in srgb, var(--border) 40%, transparent) 4px 5px
    );
  }
  /* The lines of the chain selected */
  .diff-line.commented {
    box-shadow: inset 3px 0 0 var(--yellow);
    background-image: linear-gradient(
      color-mix(in srgb, var(--yellow) 22%, transparent),
      color-mix(in srgb, var(--yellow) 22%, transparent)
    );
  }
  .diff-line.picked {
    box-shadow: inset 3px 0 0 var(--accent);
    background-color: color-mix(in srgb, var(--accent) 14%, transparent);
  }
  .diff-number {
    pointer-events: auto;
    display: flex;
    gap: var(--space-2);
    justify-content: flex-end;
    height: 100%;
    width: var(--gutter);
    padding: 0 var(--space-2) 0 0;
    border: none;
    border-right: 1px solid var(--border);
    background: color-mix(in srgb, var(--surface) 60%, transparent);
    color: var(--text-muted);
    font: inherit;
    font-family: 'SF Mono', Menlo, Consolas, monospace;
    font-size: var(--text-xs);
    user-select: none;
    /* Its text centred in one line of the code, as the code's text is in its line (ATM-652): level with it, and on a
       wrapped line's first visual line, so Wrap moves no number */
    align-items: flex-start;
    line-height: var(--line-height);
  }
  .pickable .diff-number {
    cursor: pointer;
  }
  .diff-number span {
    min-width: var(--space-9);
    text-align: right;
  }
  .diff-number:hover,
  .picked .diff-number {
    color: var(--accent);
  }
</style>
