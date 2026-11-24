<script lang="ts">
  import { untrack } from 'svelte';
  import type { ProjectDocument } from '../documents';
  import type { DocumentSource } from '../documents/source';
  import ChainBox from './ChainBox.svelte';
  import ComposerActions from './ComposerActions.svelte';
  import DiffCode, { type CodeLine } from './DiffCode.svelte';
  import { draftActions } from './drafts';
  import { focusOnShow, focusWhenPlaced } from './focus';
  import RenderedSide from './RenderedSide.svelte';
  import {
    changeStarts,
    changedLines,
    foldKey,
    nextChange,
    parseHunks,
    splitBlocks,
    unifiedBlocks,
    type Block,
    type SplitRow,
    type UnifiedRow,
  } from './diff';
  import type { Chain, Picked, Side } from './types';
  import { forgetUnsent, keepsUnsent } from './unsent';

  /** How many rows of a file are drawn at a time (ATM-587): 300 made a frame of about 120 ms when scrolled into a
   * long file, 100 about 30 */
  const CHUNK = 100;

  // The diff viewer (docs/code-review-design.md §3.7, ATM-573): one file of a round, its two sides read from document
  // sources (the round's base and head; none for a side the file doesn't have) and its hunks from the round's patch.
  // Split or unified; source (each run of lines a code block CompMD renders) or rendered (a Markdown file rendered on
  // the document viewer's page, with its line numbers, the changed lines marked: both sides in split, the new one in
  // unified). Unchanged runs fold (*show more* opens one). Pressing a line's + (or its number) picks it, dragging
  // extends the pick, Shift-press extends it too; the comment is written right under the lines (selecting text in the
  // rendered view does the same). The file's chains show in place, after their last line. Keys (`keyboard`, when the
  // viewer is the page's only one): n / p the next and previous change, ] / [ the next and previous file, v viewed.
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
    wrap = false,
    viewed = false,
    header = true,
    keyboard = true,
    busy = false,
    docStyle = null,
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
    /** Long lines wrap (ATM-588); in Split each line as tall as the taller of its two sides, so they stay level
     * (ATM-616, CompMD 0.8.0's line heights) */
    wrap?: boolean;
    viewed?: boolean;
    /** Its own header (the path, the layout and view switches, Viewed); the review's page has its own */
    header?: boolean;
    /** Its keys on the window: off where the page has several viewers and the keys itself */
    keyboard?: boolean;
    busy?: boolean;
    /** The document style the reader picked in Settings, for the rendered view's page (none: the theme's) */
    docStyle?: string | null;
    /** A comment on the lines picked; why it wasn't saved, if it wasn't (the box keeps it) */
    oncomment?: (picked: Picked, body: string) => Promise<string | null> | void;
    onreply?: (to: string, body: string) => void;
    onresolve?: (comment: string, resolved: boolean) => void;
    onviewed?: () => void;
    onnextfile?: () => void;
    onprevfile?: () => void;
  } = $props();

  // The two sides, kept while the same ones load again (a refresh), and while the same file's next revisions load (a
  // live review's working copy is a new tree at each change, ATM-687): what is shown stays until the new text is
  // there, and is drawn again only where that text differs. Nothing flickers
  let oldDoc = $state.raw<ProjectDocument | null>(null);
  let newDoc = $state.raw<ProjectDocument | null>(null);
  let problem = $state<string | null>(null);
  let loaded = $state(false);
  /** The files (old path and path) whose sides are shown */
  let loadedFiles = '';

  async function side(source: DocumentSource | null, file: string) {
    if (!source) return null;
    const result = await source.load(file);
    if ('error' in result) throw new Error(result.message);
    return 'document' in result ? result.document : null;
  }

  $effect(() => {
    const [b, a, from, to, p] = [before, after, oldPath, path, patch];
    const files = `${from}|${to}`;
    // Another file (not another revision of this one): loading until its sides are there
    if (files !== loadedFiles) {
      loaded = false;
      problem = null;
    }
    void Promise.all([side(b, from), side(a, to)])
      .then(([o, n]) => {
        if (b !== before || a !== after || p !== patch) return; // another file or revision meanwhile
        if (oldDoc?.text !== o?.text || oldDoc?.path !== o?.path) oldDoc = o;
        if (newDoc?.text !== n?.text || newDoc?.path !== n?.path) newDoc = n;
        shownPatch = p;
        problem = null;
        loaded = true;
        loadedFiles = files;
      })
      .catch((e: unknown) => {
        problem = e instanceof Error ? e.message : String(e);
        loaded = true;
        loadedFiles = files;
      });
  });

  /** The patch of the sides shown: a new one waits for its sides, so the rows never pair one with the other's text */
  let shownPatch = $state(untrack(() => patch));
  const hunks = $derived(parseHunks(shownPatch));
  const language = $derived(newDoc?.language ?? oldDoc?.language ?? null);
  const markdown = $derived((newDoc ?? oldDoc)?.kind === 'markdown');
  const shownView = $derived(view === 'rendered' && markdown ? 'rendered' : 'source');
  const newText = $derived(newDoc?.text ?? oldDoc?.text ?? null);

  let expanded = $state(new Set<number>());
  // Another file: its folds start closed, nothing picked (the same file given again, a refresh, keeps them)
  let shownPath = untrack(() => path);
  $effect(() => {
    if (path === shownPath) return;
    shownPath = path;
    untrack(() => {
      expanded = new Set();
      picked = null;
      renderedPick = null;
      current = null;
    });
  });

  const blocks = $derived<Block<SplitRow | UnifiedRow>[]>(
    mode === 'split' ? splitBlocks(hunks, newText, expanded) : unifiedBlocks(hunks, newText, expanded),
  );
  const starts = $derived(changeStarts(blocks));
  const startKeys = $derived(new Set(starts.map(([b, r]) => `${b}-${r}`)));

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
  // Rendered (ATM-651): a chain with lines on a side shown is under its block on the page; the others (a whole
  // file's, one on a side not shown) after the document
  const elsewhere = $derived(
    shownView === 'source'
      ? chains.filter((c) => !placed.has(c.id))
      : chains.filter((c) => !c.anchor.lines || !sides.includes(c.anchor.side)),
  );

  /** A block's rows cut after each row a chain ends at, so the chain shows right under its lines, and after the last
   * line picked, so the comment being written shows right under them (inside another chain's lines too); the chains
   * ending at one row, the one starting latest (the narrowest) first */
  function segments<R extends SplitRow | UnifiedRow>(rows: R[], block: number) {
    const out: { rows: R[]; first: number; chains: Chain[] }[] = [];
    const cut = picked && picked.block === block && !dragging ? picked.to : -1;
    let first = 0;
    rows.forEach((row, i) => {
      const here = chainsAfter(row).sort((x, y) => (y.anchor.lines?.[0] ?? 0) - (x.anchor.lines?.[0] ?? 0));
      // A long run is cut every CHUNK rows too, so it can be drawn a chunk at a time (ATM-587)
      if (here.length || i === cut || i === rows.length - 1 || i - first + 1 >= CHUNK) {
        out.push({ rows: rows.slice(first, i + 1), first, chains: here });
        first = i + 1;
      }
    });
    return out;
  }

  // A long file drawn a chunk at a time (ATM-587): its first CHUNK rows at once, the rest a chunk a frame once the
  // page is idle, so making a file of thousands of rows is never one long task; another file starts again
  type Item =
    | { kind: 'fold'; b: number; fold: Extract<Block<SplitRow | UnifiedRow>, { type: 'fold' }>['fold']; start: number }
    | { kind: 'rows'; b: number; segment: ReturnType<typeof segments<SplitRow | UnifiedRow>>[number]; start: number };
  const items = $derived.by(() => {
    const out: Item[] = [];
    let start = 0;
    blocks.forEach((block, b) => {
      if (block.type === 'fold') out.push({ kind: 'fold', b, fold: block.fold, start });
      else
        for (const segment of segments(block.rows as (SplitRow | UnifiedRow)[], b)) {
          out.push({ kind: 'rows', b, segment, start });
          start += segment.rows.length;
        }
    });
    return { out, total: start };
  });
  let limit = $state(CHUNK);
  $effect(() => {
    void path;
    untrack(() => (limit = CHUNK));
  });
  $effect(() => {
    const total = items.total;
    if (limit >= total) return;
    const more = () => (limit += CHUNK);
    if (typeof requestIdleCallback === 'function') {
      const id = requestIdleCallback(more, { timeout: 200 });
      return () => cancelIdleCallback(id);
    }
    const id = setTimeout(more, 16);
    return () => clearTimeout(id);
  });
  const drawnItems = $derived(items.out.filter((item) => item.start < limit));

  // Wrapped in Split (ATM-616): each side of a run says how tall each of its lines is as drawn, and both sides make
  // each line the taller of the two, so a long line wrapped on one side keeps the other side's next line level with it
  let wrappedLines = $state<Record<string, number[]>>({});
  function tallest(run: string): number[] | null {
    const old = wrappedLines[`${run}-old`];
    const now = wrappedLines[`${run}-new`];
    if (!old && !now) return null;
    return Array.from({ length: Math.max(old?.length ?? 0, now?.length ?? 0) }, (_, i) =>
      Math.max(old?.[i] ?? 0, now?.[i] ?? 0),
    );
  }
  $effect(() => {
    if (!wrap) wrappedLines = {};
  });

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
  /** The lines of a segment that start a change, by their index in it */
  const changesIn = (block: number, first: number, count: number) =>
    Array.from({ length: count }, (_, i) => i).filter((i) => startKeys.has(`${block}-${first + i}`));

  // Picking lines: a press on a line's + or number, dragged or Shift-pressed further, in one run of one side; a run is
  // a block's rows, the chains shown between them included (`from` and `to` are rows of the block, `at` where the
  // pick began)
  let picked = $state<{ block: number; side: Side; at: number; from: number; to: number } | null>(null);
  let dragging = $state(false);
  let draft = $state('');

  function pick(block: number, s: Side, row: number, extend: boolean) {
    const p = picked;
    if (extend && p && p.block === block && p.side === s) {
      picked = { ...p, from: Math.min(p.from, row), to: Math.max(p.to, row) };
    } else {
      picked = { block, side: s, at: row, from: row, to: row };
      draft = '';
      forgetUnsent(`comment:${path}`);
    }
    dragging = true;
  }

  /** While dragging: the pick runs from where it began to the line under the pointer */
  function over(block: number, s: Side, row: number) {
    const p = picked;
    if (!dragging || !p || p.block !== block || p.side !== s) return;
    const [from, to] = [Math.min(p.at, row), Math.max(p.at, row)];
    if (from !== p.from || to !== p.to) picked = { ...p, from, to };
  }

  /** The rows of a segment (its first row, how many) picked on side `s`, as the segment's own indices */
  function pickedIn(block: number, first: number, count: number, s: Side): [number, number] | null {
    const p = picked;
    if (!p || p.block !== block || p.side !== s) return null;
    const from = Math.max(p.from, first);
    const to = Math.min(p.to, first + count - 1);
    return from <= to ? [from - first, to - first] : null;
  }

  /** What is picked, as a chain's anchor takes it: the side, its first and last line, and their text */
  const pickedLines = $derived.by((): Picked | null => {
    const p = picked;
    const block = p ? blocks[p.block] : null;
    if (!p || block?.type !== 'rows') return null;
    const rows = (block.rows as (SplitRow | UnifiedRow)[]).slice(p.from, p.to + 1);
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

  // The rendered view's pick: the lines picked by their + (ATM-650) or under the text selected. Its comment box goes
  // where the comment will show (ATM-651): in a balloon in the margin beside its lines, as Word's (RenderedSide); not
  // after the document, where a comment on its first lines was written a document away (ATM-627)
  let renderedPick = $state<Picked | null>(null);
  const composing = $derived(shownView === 'rendered' ? renderedPick : dragging ? null : pickedLines);

  function cancel() {
    forgetUnsent(`comment:${path}`);
    picked = null;
    renderedPick = null;
    draft = '';
    refused = null;
    saving = false;
  }

  // Sent: the box stays, with what was written, until the page says it is saved; a refusal shows in it
  let saving = $state(false);
  let refused = $state<string | null>(null);
  async function save(asDraft = false) {
    const body = draft.trim();
    if (!body || !composing || saving) return;
    saving = true;
    refused = null;
    const problem =
      asDraft && drafts ? await drafts.comment(path, composing, body) : await oncomment?.(composing, body);
    saving = false;
    if (typeof problem === 'string') refused = problem;
    else cancel();
  }
  /** What drafting does on this page (ATM-612); none where drafts can't be kept */
  const drafts = draftActions();
  /** The form's submit and ⌘ Enter: the main action, adding a draft while the user has some */
  const drafting = $derived(!!drafts && drafts.pending > 0);
  const submit = () => void save(drafting);

  // The chain selected (hovered, focused, or clicked to keep it): its lines are marked
  let hovered = $state<string | null>(null);
  let kept = $state<string | null>(null);
  const selectedId = $derived(hovered ?? kept);
  const selectedChain = $derived(chains.find((c) => c.id === selectedId) ?? null);
  function select(id: string, on: boolean, keep: boolean) {
    if (keep) kept = kept === id ? null : id;
    else hovered = on ? id : hovered === id ? null : hovered;
  }
  /** The rows of a segment the selected chain is on, on side `s`, as the segment's own indices */
  function markedIn(rows: (SplitRow | UnifiedRow)[], s: Side | 'both'): [number, number] | null {
    const c = selectedChain;
    if (!c?.anchor.lines) return null;
    const [from, to] = c.anchor.lines;
    const on = (r: SplitRow | UnifiedRow) => {
      if (s !== 'both' && s !== c.anchor.side) return false;
      const n = c.anchor.side === 'old' ? r.old : r.new;
      return n !== null && n >= from && n <= to;
    };
    const first = rows.findIndex(on);
    if (first < 0) return null;
    let last = first;
    while (last + 1 < rows.length && on(rows[last + 1])) last++;
    return [first, last];
  }

  const whereLabel = (p: Picked) =>
    (p.lines[0] === p.lines[1] ? `Line ${p.lines[0]}` : `Lines ${p.lines[0]}–${p.lines[1]}`) +
    (p.side === 'old' ? ' (before)' : '');

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
    if (!keyboard || e.metaKey || e.ctrlKey || e.altKey) return;
    // Not while typing (the window itself, when nothing has the focus, is no element)
    const target = e.target instanceof Element ? e.target : null;
    if (target?.closest('input, textarea, select, [contenteditable="true"]')) return;
    const act = { n: () => go(false), p: () => go(true), ']': onnextfile, '[': onprevfile, v: onviewed }[e.key];
    if (!act) return;
    e.preventDefault();
    act();
  }

  // The rendered view: both sides in split, the new one (the old, for a file deleted) in unified
  const changed = $derived(changedLines(hunks));
  const sides = $derived<Side[]>(
    mode === 'unified'
      ? newDoc
        ? ['new']
        : oldDoc
          ? ['old']
          : []
      : [oldDoc ? ('old' as const) : null, newDoc ? ('new' as const) : null].filter((s): s is Side => s !== null),
  );
</script>

<svelte:window {onkeydown} onmouseup={() => (dragging = false)} />

{#snippet chainBox(chain: Chain)}
  <ChainBox
    {chain}
    {onreply}
    {onresolve}
    selected={selectedId === chain.id}
    onselect={(on, keep) => select(chain.id, on, keep)}
  />
{/snippet}

{#snippet renderedComposer()}
  {#if renderedPick}{@render composer(renderedPick)}{/if}
{/snippet}

{#snippet composer(p: Picked)}
  <form
    class="diff-composer"
    onsubmit={(e) => {
      e.preventDefault();
      submit();
    }}
  >
    <div class="diff-composer-where">
      <span class="diff-composer-plus" aria-hidden="true">+</span>
      {whereLabel(p)}
    </div>
    <textarea
      class="diff-composer-body"
      aria-label="Comment on {whereLabel(p).toLowerCase()}"
      rows="3"
      placeholder="Leave a comment…"
      bind:value={draft}
      {@attach keepsUnsent(`comment:${path}`, shownView === 'rendered' ? focusWhenPlaced : focusOnShow)}
      onkeydown={(e) => {
        if (e.key === 'Escape') cancel();
        if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) submit();
      }}></textarea>
    {#if refused}<p class="diff-composer-problem" role="alert">Not saved: {refused}</p>{/if}
    <div class="diff-composer-actions">
      <span class="diff-composer-hint">⌘ Enter to {drafting ? 'add to your review' : 'comment'}</span>
      <ComposerActions
        ready={!!draft.trim() && !busy && !saving}
        {drafting}
        now={() => void save(false)}
        oncancel={cancel}
        ondraft={drafts ? () => void save(true) : undefined}
      />
    </div>
  </form>
{/snippet}

<div class="diff-view" class:framed={header} bind:this={root} data-mode={mode} data-view={shownView} data-path={path}>
  {#if header}
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
  {/if}

  {#if problem}
    <p class="diff-problem" role="alert">{problem}</p>
  {:else if !loaded}
    <p class="diff-loading">Loading…</p>
  {:else if shownView === 'source'}
    {#each drawnItems as item (`${item.b}-${item.kind === 'fold' ? 'fold' : item.segment.first}`)}
      {@const b = item.b}
      {#if item.kind === 'fold'}
        {@const fold = item.fold}
        <button
          class="diff-fold"
          onclick={() => (expanded = new Set([...expanded, foldKey(fold)]))}
          data-fold={fold.newFrom}
        >
          Show {fold.count} unchanged {fold.count === 1 ? 'line' : 'lines'}
        </button>
      {:else}
        {@const segment = item.segment}
        {@const keys = segment.rows.map((_, i) => `${b}-${segment.first + i}`)}
        {@const isPicked = (s: Side) => pickedIn(b, segment.first, segment.rows.length, s)}
        {@const changes = changesIn(b, segment.first, segment.rows.length)}
        <div class="diff-run" data-keys={keys.join(' ')}>
          {#if mode === 'split'}
            {#each ['old', 'new'] as const as s (s)}
              {@const run = `${b}-${segment.first}`}
              <div class="diff-column diff-{s}">
                <DiffCode
                  lines={splitSide(segment.rows as SplitRow[], s)}
                  {language}
                  {wrap}
                  heights={wrap ? tallest(run) : null}
                  onwrapped={(heights) => (wrappedLines = { ...wrappedLines, [`${run}-${s}`]: heights })}
                  numbers={s}
                  {keys}
                  changes={s === 'new' ? changes : []}
                  picked={isPicked(s)}
                  marked={markedIn(segment.rows, s)}
                  onpick={oncomment ? (i, extend) => pick(b, s, segment.first + i, extend) : undefined}
                  onover={(i) => over(b, s, segment.first + i)}
                />
              </div>
            {/each}
          {:else}
            <div class="diff-column">
              <DiffCode
                lines={unifiedLines(segment.rows as UnifiedRow[])}
                {language}
                {wrap}
                numbers="both"
                {keys}
                {changes}
                picked={isPicked('new')}
                marked={markedIn(segment.rows, 'both')}
                onpick={oncomment ? (i, extend) => pick(b, 'new', segment.first + i, extend) : undefined}
                onover={(i) => over(b, 'new', segment.first + i)}
              />
            </div>
          {/if}
        </div>
        {#if composing && picked && picked.block === b && picked.to >= segment.first && picked.to < segment.first + segment.rows.length && oncomment}
          {@render composer(composing)}
        {/if}
        {#each segment.chains as chain (chain.id)}{@render chainBox(chain)}{/each}
      {/if}
    {/each}
    {#if limit < items.total}
      <div class="diff-more" style:height="{(items.total - limit) * 20}px" aria-hidden="true"></div>
    {/if}
  {:else}
    <div class="diff-rendered" class:single={sides.length < 2}>
      {#each sides as s (s)}
        {@const doc = s === 'old' ? oldDoc : newDoc}
        {#if doc}
          <RenderedSide
            {doc}
            side={s}
            changed={s === 'old' ? changed.old : changed.new}
            selected={selectedChain?.anchor.side === s ? selectedChain.anchor.lines : null}
            selectedChain={selectedId}
            picking={renderedPick?.side === s ? renderedPick.lines : null}
            {docStyle}
            chains={chains.filter((c) => c.anchor.side === s && c.anchor.lines)}
            {chainBox}
            composer={renderedPick?.side === s && oncomment ? renderedComposer : undefined}
            label={sides.length < 2
              ? s === 'old'
                ? 'Before (deleted)'
                : 'After, the changes marked'
              : s === 'old'
                ? 'Before'
                : 'After'}
            onpick={oncomment
              ? (p) => {
                  renderedPick = p;
                  draft = '';
                  forgetUnsent(`comment:${path}`);
                }
              : undefined}
          />
        {/if}
      {/each}
    </div>
  {/if}

  {#if loaded && !problem && elsewhere.length}
    <div class="diff-elsewhere">
      {#each elsewhere as chain (chain.id)}{@render chainBox(chain)}{/each}
    </div>
  {/if}
</div>

<style>
  .diff-view {
    background: var(--bg);
  }
  .diff-view.framed {
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
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
    /* Over anything a run before it could draw past its end (ATM-590: a scroll bar laid over the content) */
    position: relative;
    z-index: 1;
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
  /* The comment being written, right under its lines */
  .diff-composer {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    margin: var(--space-4) var(--space-6) var(--space-4) var(--space-10);
    padding: var(--space-5) var(--space-7);
    border: 1px solid var(--accent);
    border-left-width: 3px;
    border-radius: var(--radius-lg);
    background: var(--surface);
    box-shadow: 0 4px 14px color-mix(in srgb, var(--overlay) 18%, transparent);
    font-family: var(--font-sans);
    font-size: var(--text-md);
  }
  .diff-composer-problem {
    margin: 0;
    color: var(--red);
    font-size: var(--text-sm);
  }
  .diff-composer-hint {
    margin-right: auto;
    color: var(--text-muted);
    font-size: var(--text-xs);
  }
  .diff-composer-where {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    color: var(--text-muted);
    font-size: var(--text-xs);
  }
  .diff-composer-plus {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--space-7);
    height: var(--space-7);
    border-radius: var(--radius-sm);
    background: var(--accent);
    color: var(--on-accent);
    font-weight: 700;
  }
  .diff-composer-body {
    width: 100%;
    box-sizing: border-box;
    padding: var(--space-4) var(--space-5);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg);
    color: var(--text);
    font: inherit;
    resize: vertical;
  }
  .diff-composer-body:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 20%, transparent);
  }
  /* Wrapping, whole buttons, when the box is narrow (a rendered document's margin, ATM-651) */
  .diff-composer-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: var(--space-3);
  }
  /* Its buttons are ComposerActions' (ATM-612) */
  .diff-composer-actions :global(button) {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-6);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    font-weight: 500;
    background: var(--bg);
    color: var(--text);
    font: inherit;
    white-space: nowrap;
    cursor: pointer;
  }
  .diff-composer-actions :global(button.primary) {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  .diff-composer-actions :global(button:disabled) {
    opacity: 0.5;
    cursor: default;
  }
  /* The two sides, their pages as wide as each other: a side with comments (ATM-651) takes its margin's room on top of
     its share, so its page isn't the narrower one. Its own stacking context: the comment box is above the pages, their
     marks and folds, and under the card's sticky header */
  .diff-rendered {
    position: relative;
    isolation: isolate;
    display: flex;
    align-items: stretch;
  }
  .diff-rendered > :global(.diff-pane) {
    flex: 1 1 0%;
  }
  .diff-rendered > :global(.diff-pane.with-margin) {
    flex: 1 1 var(--doc-margin-room);
  }
  .diff-rendered :global(.diff-pane.diff-old) {
    border-right: 1px solid var(--border);
  }
  .diff-rendered.single :global(.diff-pane.diff-old) {
    border-right: none;
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
