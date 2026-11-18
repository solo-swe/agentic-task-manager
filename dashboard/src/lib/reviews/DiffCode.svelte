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
    onpick,
    onover,
  }: {
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
  const text = $derived(fenced(source, language));

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
    const style = getComputedStyle(code);
    const lineHeight = parseFloat(style.lineHeight) || parseFloat(style.fontSize) * 1.6 || 20;
    const top = code.getBoundingClientRect().top - el.getBoundingClientRect().top;
    if (layout?.top !== top || layout.lineHeight !== lineHeight) layout = { top, lineHeight };
  }

  $effect(() => {
    const target = el;
    if (!target) return;
    measure();
    const resize = new ResizeObserver(measure);
    resize.observe(target);
    const rendered = new MutationObserver(measure);
    rendered.observe(target, { childList: true, subtree: true, attributes: true, attributeFilter: ['data-compmd'] });
    return () => {
      resize.disconnect();
      rendered.disconnect();
    };
  });

  /** The line at a pointer's height in the run, if there is one there */
  function lineAt(e: MouseEvent): number | null {
    if (!el || !layout) return null;
    const y = e.clientY - el.getBoundingClientRect().top - layout.top;
    const i = Math.floor(y / layout.lineHeight);
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
  class:both={numbers === 'both'}
  class:pickable={!!onpick}
  bind:this={el}
  data-lines={lines.length}
  onmousemove={move}
  onmouseleave={() => (hover = null)}
>
  {#if layout}
    {#each lines as line, i (i)}
      <div
        class="diff-line diff-{line.kind}"
        class:picked={isPicked(i)}
        class:commented={!!marked && i >= marked[0] && i <= marked[1]}
        data-row={keys[i]}
        data-change={starts.has(i) ? '' : undefined}
        style:top="{layout.top + i * layout.lineHeight}px"
        style:height="{layout.lineHeight}px"
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
            <button
              class="diff-plus"
              class:held={isPicked(i)}
              tabindex="-1"
              aria-label="Comment on line {line.new ?? line.old}"
              onmousedown={(e) => press(e, i)}>+</button
            >
          {/if}
        {/if}
      </div>
    {/each}
  {/if}
  <!-- Plain until its turn: the page's runs are highlighted one at a time, nearest the view first (ATM-587) -->
  <CompMd {text} plain={source} />
</div>

<style>
  .diff-code {
    --gutter: var(--space-10);
    --plus: var(--space-8);
    position: relative;
    padding-left: calc(var(--gutter) + var(--plus));
  }
  .diff-code.both {
    --gutter: calc(var(--space-10) * 2);
  }
  .diff-code :global(.compmd-page pre) {
    margin: 0;
    white-space: pre;
    overflow-x: auto;
    background: transparent;
  }
  .diff-code :global(.compmd-page pre > code) {
    display: block;
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
  /* The + of the line under the pointer, and of the lines picked while their comment is written */
  .diff-plus {
    pointer-events: auto;
    position: absolute;
    top: 50%;
    left: calc(var(--gutter) + var(--space-1));
    transform: translateY(-50%);
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--space-7);
    height: var(--space-7);
    max-height: 100%;
    padding: 0;
    border: none;
    border-radius: var(--radius-sm);
    background: var(--accent);
    color: var(--on-accent);
    font: inherit;
    font-weight: 700;
    font-size: var(--text-sm);
    line-height: 1;
    cursor: pointer;
    user-select: none;
  }
  .diff-plus:not(.held) {
    opacity: 0.85;
  }
</style>
