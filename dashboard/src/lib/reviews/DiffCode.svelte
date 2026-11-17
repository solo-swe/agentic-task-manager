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
  // (the line numbers, a button each: a click picks the line, Shift extends the pick) and the change marks beside it,
  // placed where CompMD drew the lines (their top and line height, measured after each render and on resize).
  let {
    lines,
    language,
    numbers = 'new',
    keys = [],
    picked = null,
    onpick,
  }: {
    lines: CodeLine[];
    language: string | null;
    /** Each line's key in the viewer (its block and row), so the viewer's n / p can find it */
    keys?: string[];
    /** Which numbers the gutter shows: one side's, or both (the unified view) */
    numbers?: 'old' | 'new' | 'both';
    /** The lines picked, by their index in `lines` */
    picked?: [number, number] | null;
    onpick?: (index: number, extend: boolean) => void;
  } = $props();

  const text = $derived(fenced(lines.map((l) => l.text).join('\n'), language));

  let el = $state<HTMLElement>();
  let layout = $state<{ top: number; lineHeight: number } | null>(null);

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

  const isPicked = (i: number) => !!picked && i >= picked[0] && i <= picked[1];
  const label = (l: CodeLine) =>
    numbers === 'both' ? [l.old ?? '', l.new ?? ''] : [(numbers === 'old' ? l.old : l.new) ?? ''];
</script>

<div class="diff-code" class:both={numbers === 'both'} bind:this={el} data-lines={lines.length}>
  {#if layout}
    {#each lines as line, i (i)}
      <div
        class="diff-line diff-{line.kind}"
        class:picked={isPicked(i)}
        data-row={keys[i]}
        style:top="{layout.top + i * layout.lineHeight}px"
        style:height="{layout.lineHeight}px"
      >
        {#if line.kind !== 'pad'}
          <button
            class="diff-number"
            tabindex="-1"
            aria-label="Line {line.new ?? line.old}"
            onclick={(e) => onpick?.(i, e.shiftKey)}
            >{#each label(line) as n, k (k)}<span>{n}</span>{/each}</button
          >
        {/if}
      </div>
    {/each}
  {/if}
  <CompMd {text} />
</div>

<style>
  .diff-code {
    position: relative;
    padding-left: var(--space-10);
  }
  .diff-code.both {
    padding-left: calc(var(--space-10) * 2);
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
    width: var(--space-10);
    padding: 0 var(--space-2) 0 0;
    border: none;
    background: none;
    color: var(--text-muted);
    font: inherit;
    font-family: 'SF Mono', Menlo, Consolas, monospace;
    font-size: var(--text-xs);
    cursor: pointer;
  }
  .both .diff-number {
    width: calc(var(--space-10) * 2);
  }
  .diff-number span {
    min-width: var(--space-9);
    text-align: right;
  }
  .diff-number:hover {
    color: var(--accent);
  }
</style>
