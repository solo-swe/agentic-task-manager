<script lang="ts">
  import { untrack } from 'svelte';
  import CompMd from '../documents/CompMd.svelte';
  import type { ProjectDocument } from '../documents';
  import type { DocumentSource } from '../documents/source';
  import type { DrawnLine } from '../compmd';
  import ChainBox from './ChainBox.svelte';
  import DiffCode, { type CodeLine } from './DiffCode.svelte';
  import {
    changeStarts,
    changedLines,
    foldKey,
    linesUnder,
    nextChange,
    parseHunks,
    splitBlocks,
    unifiedBlocks,
    type Block,
    type SplitRow,
    type UnifiedRow,
  } from './diff';
  import type { Chain, Picked, Side } from './types';

  // The diff viewer (docs/code-review-design.md §3.7, ATM-573): one file of a round, its two sides read from document
  // sources (the round's base and head; none for a side the file doesn't have) and its hunks from the round's patch.
  // Split or unified; source (each run of lines a code block CompMD renders) or rendered (a Markdown file, both sides
  // rendered, the changed blocks marked by CompMD's source-line map). Unchanged runs fold (*show more* opens one).
  // Keys: n / p the next and previous change, ] / [ the next and previous file, v viewed. Picking lines (or selecting
  // text in the rendered view) offers a comment on them; the file's chains show in place, after their last line.
  // What a comment, a reply, a resolve or another file does is the page's: the handlers.
  let {
    path,
    oldPath = path,
    before,
    after,
    patch,
    chains = [],
    mode = $bindable('split'),
    view = $bindable('source'),
    viewed = false,
    oncomment,
    onreply,
    onresolve,
    onviewed,
    onnextfile,
    onprevfile,
  }: {
    path: string;
    oldPath?: string;
    before: DocumentSource | null;
    after: DocumentSource | null;
    patch: string;
    chains?: Chain[];
    mode?: 'split' | 'unified';
    view?: 'source' | 'rendered';
    viewed?: boolean;
    oncomment?: (picked: Picked) => void;
    onreply?: (to: string, body: string) => void;
    onresolve?: (comment: string, resolved: boolean) => void;
    onviewed?: () => void;
    onnextfile?: () => void;
    onprevfile?: () => void;
  } = $props();

  // The two sides
  let oldDoc = $state.raw<ProjectDocument | null>(null);
  let newDoc = $state.raw<ProjectDocument | null>(null);
  let problem = $state<string | null>(null);
  let loaded = $state(false);

  async function side(source: DocumentSource | null, file: string) {
    if (!source) return null;
    const result = await source.load(file);
    if ('error' in result) throw new Error(result.message);
    return 'document' in result ? result.document : null;
  }

  $effect(() => {
    const [b, a, from, to] = [before, after, oldPath, path];
    loaded = false;
    problem = null;
    void Promise.all([side(b, from), side(a, to)])
      .then(([o, n]) => {
        if (b !== before || a !== after) return; // another file meanwhile
        oldDoc = o;
        newDoc = n;
        loaded = true;
      })
      .catch((e: unknown) => {
        problem = e instanceof Error ? e.message : String(e);
        loaded = true;
      });
  });

  const hunks = $derived(parseHunks(patch));
  const language = $derived(newDoc?.language ?? oldDoc?.language ?? null);
  const markdown = $derived((newDoc ?? oldDoc)?.kind === 'markdown');
  const shownView = $derived(view === 'rendered' && markdown ? 'rendered' : 'source');
  const newText = $derived(newDoc?.text ?? oldDoc?.text ?? null);

  let expanded = $state(new Set<number>());
  // Another file: its folds start closed, nothing picked
  $effect(() => {
    void path;
    untrack(() => {
      expanded = new Set();
      picked = null;
      current = null;
    });
  });

  const blocks = $derived<Block<SplitRow | UnifiedRow>[]>(
    mode === 'split' ? splitBlocks(hunks, newText, expanded) : unifiedBlocks(hunks, newText, expanded),
  );
  const starts = $derived(changeStarts(blocks));

  /** Where a chain goes: after the row of its last line, on its side */
  const endsAt = (row: SplitRow | UnifiedRow, s: Side) =>
    chains.filter(
      (c) => c.anchor.lines && c.anchor.side === s && (s === 'old' ? row.old : row.new) === c.anchor.lines[1],
    );
  const chainsAfter = (row: SplitRow | UnifiedRow) => [...endsAt(row, 'old'), ...endsAt(row, 'new')];
  /** Chains of the file that no row shown places: a whole file's, or on lines folded away */
  const placed = $derived(
    new Set(blocks.flatMap((b) => (b.type === 'rows' ? b.rows.flatMap((r) => chainsAfter(r).map((c) => c.id)) : []))),
  );
  const elsewhere = $derived(shownView === 'source' ? chains.filter((c) => !placed.has(c.id)) : chains);

  /** A block's rows cut after each row a chain ends at, so the chain shows right under its lines */
  function segments<R extends SplitRow | UnifiedRow>(rows: R[]) {
    const out: { rows: R[]; first: number; chains: Chain[] }[] = [];
    let first = 0;
    rows.forEach((row, i) => {
      const here = chainsAfter(row);
      if (here.length || i === rows.length - 1) {
        out.push({ rows: rows.slice(first, i + 1), first, chains: here });
        first = i + 1;
      }
    });
    return out;
  }

  const splitSide = (rows: SplitRow[], s: Side): CodeLine[] =>
    rows.map((r) => {
      const text = (s === 'old' ? r.oldText : r.newText) ?? '';
      const kind =
        r.kind === 'context' || r.kind === 'changed'
          ? r.kind
          : (s === 'old') === (r.kind === 'removed')
            ? r.kind
            : 'pad';
      return { text, old: s === 'old' ? r.old : null, new: s === 'new' ? r.new : null, kind };
    });
  const unifiedLines = (rows: UnifiedRow[]): CodeLine[] => rows.map((r) => ({ ...r }));

  // Picking lines: a click on a line number, Shift extends it, in one run of one side
  let picked = $state<{ block: number; first: number; side: Side; from: number; to: number } | null>(null);

  function pick(block: number, first: number, s: Side, index: number, extend: boolean) {
    const p = picked;
    picked =
      extend && p && p.block === block && p.first === first && p.side === s
        ? { ...p, from: Math.min(p.from, index), to: Math.max(p.to, index) }
        : { block, first, side: s, from: index, to: index };
  }

  /** What is picked, as a chain's anchor takes it: the side, its first and last line, and their text */
  const pickedLines = $derived.by((): Picked | null => {
    const p = picked;
    const block = p ? blocks[p.block] : null;
    if (!p || block?.type !== 'rows') return null;
    const rows = (block.rows as (SplitRow | UnifiedRow)[]).slice(p.first + p.from, p.first + p.to + 1);
    const numbered = rows
      .map((r) => {
        const n = p.side === 'old' || r.new === null ? r.old : r.new;
        const text = 'text' in r ? r.text : p.side === 'old' ? r.oldText : r.newText;
        return { side: (p.side === 'old' || r.new === null ? 'old' : 'new') as Side, n, text };
      })
      .filter((r): r is { side: Side; n: number; text: string } => r.n !== null && r.text !== null);
    if (!numbered.length) return null;
    const s = numbered[0].side;
    const same = numbered.filter((r) => r.side === s);
    return { side: s, lines: [same[0].n, same.at(-1)!.n], quote: same.map((r) => r.text).join('\n') };
  });

  function comment() {
    if (pickedLines) oncomment?.(pickedLines);
    picked = null;
  }

  // n / p: the change after or before the current one, scrolled to; ] / [ and v: the page's
  let current = $state<[number, number] | null>(null);
  let root = $state<HTMLElement>();

  function go(back: boolean) {
    const to = nextChange(starts, current, back);
    if (!to) return;
    current = to;
    root?.querySelector(`[data-row="${to[0]}-${to[1]}"]`)?.scrollIntoView({ block: 'center' });
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    // Not while typing (the window itself, when nothing has the focus, is no element)
    const target = e.target instanceof Element ? e.target : null;
    if (target?.closest('input, textarea, select, [contenteditable="true"]')) return;
    const act = { n: () => go(false), p: () => go(true), ']': onnextfile, '[': onprevfile, v: onviewed }[e.key];
    if (!act) return;
    e.preventDefault();
    act();
  }

  // The rendered view: where CompMD drew each source line, per side, to mark the changed ones and map a selection
  const changed = $derived(changedLines(hunks));
  let drawn = $state.raw<Record<Side, DrawnLine[]>>({ old: [], new: [] });
  let panes = $state<Record<Side, HTMLElement | undefined>>({ old: undefined, new: undefined });

  function placeLines(s: Side, lines: DrawnLine[], from: HTMLElement) {
    const pane = panes[s];
    if (!pane) return;
    const box = pane.getBoundingClientRect();
    const at = from.getBoundingClientRect();
    const dx = at.left - box.left;
    const dy = at.top - box.top;
    drawn = {
      ...drawn,
      [s]: lines.map((l) => ({ line: l.line, rects: l.rects.map((r) => ({ ...r, x: r.x + dx, y: r.y + dy })) })),
    };
  }

  const marks = (s: Side) =>
    drawn[s].filter((l) => (s === 'old' ? changed.old : changed.new).has(l.line)).flatMap((l) => l.rects);

  function selectText(s: Side) {
    const pane = panes[s];
    const selection = window.getSelection();
    if (!pane || !selection || selection.isCollapsed || !oncomment) return;
    const box = pane.getBoundingClientRect();
    const rects = [...selection.getRangeAt(0).getClientRects()].map((r) => ({
      top: r.top - box.top,
      bottom: r.bottom - box.top,
    }));
    const lines = linesUnder(drawn[s], rects);
    if (lines) oncomment({ side: s, lines, quote: selection.toString() });
  }

  const sides = $derived<Side[]>(
    [oldDoc ? ('old' as const) : null, newDoc ? ('new' as const) : null].filter((s): s is Side => s !== null),
  );
</script>

<svelte:window {onkeydown} />

<div class="diff-view" bind:this={root} data-mode={mode} data-view={shownView} data-path={path}>
  <header class="diff-header">
    <span class="diff-path">{oldPath !== path ? `${oldPath} → ${path}` : path}</span>
    <div class="diff-switch" role="group" aria-label="Layout">
      <button aria-pressed={mode === 'split'} onclick={() => (mode = 'split')}>Split</button>
      <button aria-pressed={mode === 'unified'} onclick={() => (mode = 'unified')}>Unified</button>
    </div>
    {#if markdown}
      <div class="diff-switch" role="group" aria-label="View">
        <button aria-pressed={shownView === 'source'} onclick={() => (view = 'source')}>Source</button>
        <button aria-pressed={shownView === 'rendered'} onclick={() => (view = 'rendered')}>Rendered</button>
      </div>
    {/if}
    {#if onviewed}
      <label class="diff-viewed"><input type="checkbox" checked={viewed} onchange={() => onviewed()} /> Viewed</label>
    {/if}
  </header>

  {#if problem}
    <p class="diff-problem" role="alert">{problem}</p>
  {:else if !loaded}
    <p class="diff-loading">Loading…</p>
  {:else if shownView === 'source'}
    {#each blocks as block, b (b)}
      {#if block.type === 'fold'}
        <button
          class="diff-fold"
          onclick={() => (expanded = new Set([...expanded, foldKey(block.fold)]))}
          data-fold={block.fold.newFrom}
        >
          Show {block.fold.count} unchanged {block.fold.count === 1 ? 'line' : 'lines'}
        </button>
      {:else}
        {#each segments(block.rows as (SplitRow | UnifiedRow)[]) as segment (segment.first)}
          {@const keys = segment.rows.map((_, i) => `${b}-${segment.first + i}`)}
          {@const isPicked = (s: Side): [number, number] | null =>
            picked && picked.block === b && picked.first === segment.first && picked.side === s
              ? [picked.from, picked.to]
              : null}
          <div class="diff-run" data-keys={keys.join(' ')}>
            {#if mode === 'split'}
              {#each ['old', 'new'] as const as s (s)}
                <div class="diff-column diff-{s}">
                  <DiffCode
                    lines={splitSide(segment.rows as SplitRow[], s)}
                    {language}
                    numbers={s}
                    {keys}
                    picked={isPicked(s)}
                    onpick={(i, extend) => pick(b, segment.first, s, i, extend)}
                  />
                </div>
              {/each}
            {:else}
              <div class="diff-column">
                <DiffCode
                  lines={unifiedLines(segment.rows as UnifiedRow[])}
                  {language}
                  numbers="both"
                  {keys}
                  picked={isPicked('new')}
                  onpick={(i, extend) => pick(b, segment.first, 'new', i, extend)}
                />
              </div>
            {/if}
          </div>
          {#if picked && picked.block === b && picked.first === segment.first && pickedLines && oncomment}
            <div class="diff-picked">
              <span>
                {pickedLines.lines[0] === pickedLines.lines[1]
                  ? `Line ${pickedLines.lines[0]}`
                  : `Lines ${pickedLines.lines[0]}–${pickedLines.lines[1]}`}{pickedLines.side === 'old'
                  ? ' (before)'
                  : ''}
              </span>
              <button onclick={comment}>Comment</button>
              <button onclick={() => (picked = null)}>Cancel</button>
            </div>
          {/if}
          {#each segment.chains as chain (chain.id)}<ChainBox {chain} {onreply} {onresolve} />{/each}
        {/each}
      {/if}
    {/each}
  {:else}
    <div class="diff-rendered" class:single={sides.length < 2}>
      {#each sides as s (s)}
        {@const doc = s === 'old' ? oldDoc : newDoc}
        <!-- svelte-ignore a11y_no_static_element_interactions (a selection, made with the mouse, offers a comment) -->
        <div class="diff-pane diff-{s}" bind:this={panes[s]} onmouseup={() => selectText(s)}>
          <div class="diff-pane-label">{s === 'old' ? 'Before' : 'After'}</div>
          {#each marks(s) as rect, i (i)}
            <div
              class="diff-block-mark diff-{s === 'old' ? 'removed' : 'added'}"
              style:left="{rect.x}px"
              style:top="{rect.y}px"
              style:width="{rect.width}px"
              style:height="{rect.height}px"
            ></div>
          {/each}
          {#if doc}
            <CompMd
              text={doc.text ?? doc.document}
              path={doc.path}
              onlines={(lines, from) => placeLines(s, lines, from)}
            />
          {/if}
        </div>
      {/each}
    </div>
  {/if}

  {#if loaded && !problem && elsewhere.length}
    <div class="diff-elsewhere">
      {#each elsewhere as chain (chain.id)}<ChainBox {chain} {onreply} {onresolve} />{/each}
    </div>
  {/if}
</div>

<style>
  .diff-view {
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg);
  }
  .diff-header {
    position: sticky;
    top: 0;
    z-index: 1;
    display: flex;
    align-items: center;
    gap: var(--space-6);
    padding: var(--space-3) var(--space-6);
    border-bottom: 1px solid var(--border);
    background: var(--surface);
    font-size: var(--text-md);
  }
  .diff-path {
    font-family: 'SF Mono', Menlo, Consolas, monospace;
    margin-right: auto;
  }
  .diff-switch {
    display: flex;
  }
  .diff-switch button {
    padding: var(--space-1) var(--space-4);
    border: 1px solid var(--border);
    background: none;
    color: var(--text-muted);
    font: inherit;
    font-size: var(--text-xs);
    cursor: pointer;
  }
  .diff-switch button:first-child {
    border-radius: var(--radius-sm) 0 0 var(--radius-sm);
  }
  .diff-switch button:last-child {
    border-radius: 0 var(--radius-sm) var(--radius-sm) 0;
  }
  .diff-switch button[aria-pressed='true'] {
    background: var(--accent);
    color: var(--on-accent);
    border-color: var(--accent);
  }
  .diff-viewed {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--text-xs);
    color: var(--text-muted);
  }
  .diff-run {
    display: flex;
  }
  .diff-column {
    flex: 1;
    min-width: 0;
  }
  .diff-run .diff-old {
    border-right: 1px solid var(--border);
  }
  .diff-fold {
    display: block;
    width: 100%;
    padding: var(--space-2) var(--space-6);
    border: none;
    border-top: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
    background: color-mix(in srgb, var(--accent) 6%, transparent);
    color: var(--accent);
    font: inherit;
    font-size: var(--text-xs);
    text-align: left;
    cursor: pointer;
  }
  .diff-picked {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-2) var(--space-6) var(--space-2) var(--space-10);
    font-size: var(--text-xs);
    color: var(--text-muted);
  }
  .diff-rendered {
    display: grid;
    grid-template-columns: 1fr 1fr;
  }
  .diff-rendered.single {
    grid-template-columns: 1fr;
  }
  .diff-pane {
    position: relative;
    min-width: 0;
    padding: var(--space-6) var(--space-7);
  }
  .diff-rendered .diff-old {
    border-right: 1px solid var(--border);
  }
  .diff-pane-label {
    color: var(--text-muted);
    font-size: var(--text-xs);
    margin-bottom: var(--space-3);
  }
  .diff-block-mark {
    position: absolute;
    pointer-events: none;
    border-radius: var(--radius-sm);
  }
  .diff-block-mark.diff-removed {
    background: color-mix(in srgb, var(--red) 16%, transparent);
  }
  .diff-block-mark.diff-added {
    background: color-mix(in srgb, var(--green) 16%, transparent);
  }
  .diff-problem,
  .diff-loading {
    padding: var(--space-6);
    color: var(--text-muted);
  }
  .diff-elsewhere {
    border-top: 1px solid var(--border);
    padding-bottom: var(--space-4);
  }
</style>
