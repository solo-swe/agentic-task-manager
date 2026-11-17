<script lang="ts">
  import { onDestroy, onMount, tick, untrack } from 'svelte';
  import CompMd from '../documents/CompMd.svelte';
  import type { DocumentSource } from '../documents/source';
  import { readStored, writeStored } from '../prefs';
  import ChainBox from './ChainBox.svelte';
  import { focusOnShow } from './focus';
  import ReviewFile, { holdNearCards, whenIdle } from './ReviewFile.svelte';
  import {
    chainsOf,
    driftedOf,
    fileTree,
    filterChains,
    isResolved,
    lineCounts,
    reviewChains,
    treeWidth,
    TREE_WIDTH,
    type ChainFilter,
  } from './page';
  import { STATUS_LABELS, type PlacedChain, type Picked, type ReviewPageData, type ViewFile } from './types';

  // A review's page (docs/code-review-design.md §3.11, ATM-574), a page of its own: a top bar (the project's short
  // name, the reference and title, its status, *Submit review*) and three tabs. *Overview*: the summary, who and what
  // (author, reviewers, branch, tasks, rounds), the review's own chains. *Diffs*: the round picker (a round's whole
  // change, or a round against an earlier one), the files as a tree beside the list of their diffs (a splitter between
  // them, its width kept), each file a card that folds once viewed, the previous and next change buttons, the layout
  // (split, unified) and view (source, rendered) for all. *Comments*: every chain, all, open or resolved. Keys on the
  // Diffs tab: n / p the next and previous change, ] / [ the next and previous file, v the file at the top viewed.
  // Everything it does is the page's: the handlers. Which round and file are shown is the address's.
  let {
    page,
    project = null,
    file = null,
    side,
    isViewed,
    taskHref,
    backHref,
    busy = false,
    problem = null,
    onback,
    onfile,
    onround,
    onviewed,
    oncomment,
    onreply,
    onresolve,
    onsubmit,
  }: {
    page: ReviewPageData;
    /** The project's short name (its key), before the reference */
    project?: string | null;
    /** The file in view (its path): the Diffs tab opens at it */
    file?: string | null;
    /** A side's files: the commit or tree a round's side is read from */
    side: (revision: string) => DocumentSource;
    isViewed: (file: ViewFile) => boolean;
    /** A task's address, for its link */
    taskHref?: (key: string) => string;
    /** The reviews page's address, for the link back */
    backHref?: string;
    busy?: boolean;
    problem?: string | null;
    onback?: () => void;
    onfile: (path: string) => void;
    onround: (round: number, against: number | null) => void;
    onviewed: (file: ViewFile) => void;
    oncomment: (comment: { path: string | null; picked: Picked | null; body: string }) => void;
    onreply: (to: string, body: string) => void;
    onresolve: (comment: string, resolved: boolean) => void;
    onsubmit: (decision: 'approve' | 'request_changes' | 'comment', body: string) => void;
  } = $props();

  const review = $derived(page.review);
  const view = $derived(page.view);
  const files = $derived(view.files);
  const tree = $derived(fileTree(files));
  const ended = $derived(review.status === 'approved' || review.status === 'closed');
  const rounds = $derived(page.rounds.map((r) => r.number));
  const sideOf = (revision: string | null) => (revision ? side(revision) : null);
  const statusLetter = { added: 'A', modified: 'M', deleted: 'D', renamed: 'R' } as const;

  // The tab: Diffs when the address names a file, else Overview
  let tab = $state<'overview' | 'diffs' | 'comments'>('overview');
  onMount(() => {
    if (file) tab = 'diffs';
  });

  // The layout and view of every diff, and the tree's width: kept in this browser
  const LAYOUT_KEY = 'atm-review-layout';
  const stored = readStored(LAYOUT_KEY);
  let mode = $state<'split' | 'unified'>(stored.mode === 'unified' ? 'unified' : 'split');
  let rendered = $state<'source' | 'rendered'>(stored.view === 'rendered' ? 'rendered' : 'source');
  let width = $state(typeof stored.tree === 'number' ? stored.tree : TREE_WIDTH.default);
  $effect(() => writeStored(LAYOUT_KEY, { mode, view: rendered, tree: width }));
  const hasMarkdown = $derived(files.some((f) => /\.(md|markdown|mdx)$/i.test(f.path)));

  // A file folds once viewed; its fold button opens or folds it again whatever its tick
  let folds = $state<Record<string, boolean>>({});
  const folded = (f: ViewFile) => folds[f.path] ?? isViewed(f);
  function toggleViewed(f: ViewFile) {
    const rest = { ...folds };
    delete rest[f.path];
    folds = rest;
    onviewed(f);
  }
  const viewedCount = $derived(files.filter((f) => isViewed(f)).length);

  // The list of diffs, and the file at its top (the tree marks it; ] [ and v act on it)
  let list = $state<HTMLElement | null>(null);
  let atFile = $state<string | null>(null);
  const cards = () => [...(list?.querySelectorAll<HTMLElement>('.review-file-card') ?? [])];
  /** The file gone to (the tree, ] and [): it stays the one marked while its card is in view, even where the list
   * can't scroll it to the top (the last files) */
  let chosen: string | null = null;
  function findTop() {
    const box = list?.getBoundingClientRect();
    if (!box) return;
    const cardOf = (path: string) => cards().find((c) => c.dataset.file === path);
    const kept = chosen ? cardOf(chosen)?.getBoundingClientRect() : undefined;
    if (kept && kept.top >= box.top - 4 && kept.top < box.bottom) {
      atFile = chosen;
      return;
    }
    chosen = null;
    const card = cards().find((c) => c.getBoundingClientRect().bottom > box.top + 48);
    atFile = card?.dataset.file ?? null;
  }
  let scrolling = 0;
  function onscroll() {
    if (scrolling) return;
    scrolling = requestAnimationFrame(() => {
      scrolling = 0;
      findTop();
    });
  }

  let writing: (() => void) | null = null;
  onDestroy(() => writing?.());
  /** The file just gone to: its card is made at once, the others after it */
  let wanted = $state<string | null>(null);
  const cardOf = (path: string) =>
    list?.querySelector<HTMLElement>(`.review-file-card[data-file="${CSS.escape(path)}"]`) ?? null;
  async function showFile(path: string, address = true) {
    tab = 'diffs';
    holdNearCards();
    wanted = path;
    await tick();
    cardOf(path)?.scrollIntoView({ block: 'start' });
    atFile = chosen = pinned = path;
    // The address follows once the page is idle: the file is drawn first (a navigation re-renders the layout)
    writing?.();
    if (address) writing = whenIdle(() => onfile(path));
  }

  // The file gone to stays at the top while the cards around it are made (each as tall as its diff, not its
  // estimate), until the user scrolls, clicks or presses a key in the list, or another jump
  let pinned: string | null = null;
  $effect(() => {
    const el = list;
    void files;
    if (!el) return;
    const unpin = () => (pinned = null);
    const settle = new ResizeObserver(() => {
      if (pinned) cardOf(pinned)?.scrollIntoView({ block: 'start' });
    });
    for (const card of el.querySelectorAll('.review-file-card')) settle.observe(card);
    const events = ['wheel', 'touchstart', 'mousedown', 'keydown'] as const;
    for (const name of events) el.addEventListener(name, unpin, { passive: true });
    return () => {
      settle.disconnect();
      for (const name of events) el.removeEventListener(name, unpin);
    };
  });
  // The address's file, once the list is there
  let opened = false;
  $effect(() => {
    if (opened || !list || !file || !files.some((f) => f.path === file)) return;
    opened = true;
    void showFile(file, false);
  });

  function stepFile(by: number) {
    const i = files.findIndex((f) => f.path === (atFile ?? files[0]?.path));
    const next = files[i + by];
    if (next) void showFile(next.path);
  }

  // The previous and next change, across the files: the next mark of a change below (or above) the last one gone to,
  // or the top of the list; a file not made yet is made first, and its first (or last) change gone to
  let at: Element | null = null;
  /** Wait for a card's diff to show a change: its sides load first */
  async function changesOf(card: HTMLElement) {
    for (let i = 0; i < 120; i++) {
      const marks = card.querySelectorAll('[data-change]');
      if (marks.length) return [...marks];
      await new Promise((r) => requestAnimationFrame(r));
    }
    return [];
  }
  async function jump(back: boolean) {
    tab = 'diffs';
    holdNearCards();
    pinned = null;
    await tick();
    if (!list) return;
    const from = at?.isConnected ? at.getBoundingClientRect().top : list.getBoundingClientRect().top + (back ? 0 : -1);
    // In page order (a rendered split's two sides interleave)
    const marks = [...list.querySelectorAll<HTMLElement>('[data-change], .review-file-card[data-pending]')]
      .map((el) => ({ el, top: el.getBoundingClientRect().top }))
      .sort((a, b) => a.top - b.top);
    const above = marks.filter((m) => m.top < from - 2);
    let to: Element | undefined = back ? above.at(-1)?.el : marks.find((m) => m.top > from + 2)?.el;
    if (!to) return;
    if (to.matches('.review-file-card')) {
      wanted = (to as HTMLElement).dataset.file ?? null;
      to.scrollIntoView({ block: 'start' });
      const found = await changesOf(to as HTMLElement);
      to = back ? found.at(-1) : found[0];
      if (!to) return;
    }
    for (const old of list.querySelectorAll('.review-current-change')) old.classList.remove('review-current-change');
    to.classList.add('review-current-change');
    to.scrollIntoView({ block: 'center' });
    at = to;
    chosen = null;
    findTop();
  }

  function onkeydown(e: KeyboardEvent) {
    if (tab !== 'diffs' || e.metaKey || e.ctrlKey || e.altKey) return;
    const target = e.target instanceof Element ? e.target : null;
    if (target?.closest('input, textarea, select, [contenteditable="true"]')) return;
    const top = files.find((f) => f.path === (atFile ?? files[0]?.path));
    const act = {
      n: () => jump(false),
      p: () => jump(true),
      ']': () => stepFile(1),
      '[': () => stepFile(-1),
      v: () => top && toggleViewed(top),
    }[e.key];
    if (!act) return;
    e.preventDefault();
    void act();
  }

  // The splitter: dragged, or moved with the arrow keys
  let split = $state<HTMLElement>();
  /** The width the tree and the diffs share (the window's, before the page is laid out) */
  const pageWidth = () => split?.clientWidth || window.innerWidth;
  function drag(e: PointerEvent) {
    const bar = e.currentTarget as HTMLElement;
    bar.setPointerCapture(e.pointerId);
    const left = split?.getBoundingClientRect().left ?? 0;
    const move = (m: PointerEvent) => (width = treeWidth(m.clientX - left, pageWidth()));
    const up = () => {
      bar.removeEventListener('pointermove', move);
      bar.removeEventListener('pointerup', up);
    };
    bar.addEventListener('pointermove', move);
    bar.addEventListener('pointerup', up);
  }
  function nudge(e: KeyboardEvent) {
    const by = { ArrowLeft: -16, ArrowRight: 16 }[e.key];
    if (!by) return;
    e.preventDefault();
    width = treeWidth(width + by, pageWidth());
  }

  // A chain on the review as a whole
  let composing = $state(false);
  let draft = $state('');
  function saveComment() {
    const body = draft.trim();
    if (!body) return;
    oncomment({ path: null, picked: null, body });
    composing = false;
    draft = '';
  }

  // Submit: a summary and a decision
  let submitting = $state(false);
  let summary = $state('');
  function submit(decision: 'approve' | 'request_changes' | 'comment') {
    onsubmit(decision, summary.trim());
    submitting = false;
  }
  // A decision made (the review's `updated` moves): the summary box starts empty; a refresh that changed nothing else
  // keeps what is being written
  let updatedAt = untrack(() => review.updated);
  $effect(() => {
    if (review.updated === updatedAt) return;
    updatedAt = review.updated;
    summary = '';
  });

  // The Comments tab
  let filter = $state<ChainFilter>('all');
  const chainCounts = $derived({
    all: page.chains.length,
    open: page.chains.filter((c) => !isResolved(c)).length,
    resolved: page.chains.filter((c) => isResolved(c)).length,
  });
  const listed = $derived(filterChains(page.chains, filter));
  /** A chain as the Comments tab shows it: where it was made (a drifted one at its own lines) */
  const asMade = (c: PlacedChain) => ({
    ...c,
    anchor: { ...c.anchor, lines: c.placed ?? c.drifted?.lines ?? c.anchor.lines },
  });

  const tabs = $derived([
    { id: 'overview', label: 'Overview', count: null },
    { id: 'diffs', label: 'Diffs', count: files.length },
    { id: 'comments', label: 'Comments', count: page.chains.length },
  ] as const);
</script>

<svelte:window {onkeydown} />

<article class="review-page" id="review-page" data-review={review.reference} data-tab={tab} aria-busy={busy}>
  <header class="review-top">
    <div class="review-top-row">
      {#if backHref || onback}
        <a
          class="review-back"
          href={backHref ?? '#'}
          onclick={(e) => {
            if (!onback) return;
            e.preventDefault();
            onback();
          }}>← Reviews</a
        >
      {/if}
      <h1 class="review-title">
        {#if project}<span class="review-project">{project}</span>{/if}
        <span class="review-ref">{review.reference}</span>
        <span class="review-name">{review.title}</span>
      </h1>
      <span class="review-status status-{review.status}" data-status={review.status}
        >{STATUS_LABELS[review.status]}</span
      >
      {#if page.needs_you}<span class="review-needs">Needs you</span>{/if}
      {#if !ended}
        <div class="review-submit-anchor">
          <button
            class="review-submit-toggle primary"
            aria-expanded={submitting}
            onclick={() => (submitting = !submitting)}>Submit review</button
          >
          {#if submitting}
            <section class="review-submit" aria-label="Submit">
              <textarea
                id="review-summary-body"
                bind:value={summary}
                rows="4"
                placeholder="A summary of your review (needed to comment)"
                aria-label="Summary"
                {@attach focusOnShow}></textarea>
              <div class="review-actions">
                <button class="primary" data-decision="approve" disabled={busy} onclick={() => submit('approve')}
                  >Approve</button
                >
                <button data-decision="request_changes" disabled={busy} onclick={() => submit('request_changes')}
                  >Request changes</button
                >
                <button data-decision="comment" disabled={busy || !summary.trim()} onclick={() => submit('comment')}
                  >Comment</button
                >
              </div>
            </section>
          {/if}
        </div>
      {/if}
    </div>
    <div class="review-top-row review-bar">
      <div class="review-tabs" role="tablist" aria-label="Review">
        {#each tabs as t (t.id)}
          <button role="tab" id="review-tab-{t.id}" aria-selected={tab === t.id} onclick={() => (tab = t.id)}
            >{t.label}{#if t.count !== null}<span class="review-count">{t.count}</span>{/if}</button
          >
        {/each}
      </div>
      {#if tab === 'diffs'}
        <div class="review-tools">
          <label>
            Round
            <select id="review-round" value={view.round} onchange={(e) => onround(Number(e.currentTarget.value), null)}>
              {#each rounds as n (n)}<option value={n}>{n} of {rounds.length}</option>{/each}
            </select>
          </label>
          <select
            id="review-against"
            aria-label="Showing"
            value={view.against ?? ''}
            onchange={(e) => onround(view.round, e.currentTarget.value ? Number(e.currentTarget.value) : null)}
          >
            <option value="">the whole change</option>
            {#each rounds.filter((n) => n < view.round) as n (n)}<option value={n}>since round {n}</option>{/each}
          </select>
          <span class="review-progress" title="{viewedCount} of {files.length} viewed">
            <span class="review-viewed-count">{viewedCount} of {files.length} viewed</span>
            <span class="review-meter"
              ><span style:width="{files.length ? (viewedCount / files.length) * 100 : 0}%"></span></span
            >
          </span>
          <div class="review-switch" role="group" aria-label="Changes">
            <button id="review-prev-change" title="Previous change (p)" onclick={() => jump(true)}>↑ Prev</button>
            <button id="review-next-change" title="Next change (n)" onclick={() => jump(false)}>↓ Next</button>
          </div>
          <div class="review-switch" role="group" aria-label="Layout">
            <button aria-pressed={mode === 'split'} onclick={() => (mode = 'split')}>Split</button>
            <button aria-pressed={mode === 'unified'} onclick={() => (mode = 'unified')}>Unified</button>
          </div>
          {#if hasMarkdown}
            <div class="review-switch" role="group" aria-label="View">
              <button aria-pressed={rendered === 'source'} onclick={() => (rendered = 'source')}>Source</button>
              <button aria-pressed={rendered === 'rendered'} onclick={() => (rendered = 'rendered')}>Rendered</button>
            </div>
          {/if}
        </div>
      {/if}
    </div>
  </header>

  {#if problem}<p class="review-problem" role="alert">{problem}</p>{/if}

  {#if tab === 'overview'}
    <div class="review-overview" role="tabpanel" aria-labelledby="review-tab-overview">
      <section class="review-summary" aria-label="Description">
        {#if page.summary.trim()}<CompMd text={page.summary} />{:else}<p class="review-empty">No description.</p>{/if}
      </section>
      <dl class="review-facts">
        <dt>Author</dt>
        <dd>{review.author ?? 'nobody yet'}</dd>
        <dt>Decided by</dt>
        <dd>{review.reviewers.length ? review.reviewers.join(', ') : 'anyone but its author'}</dd>
        {#if page.branch}<dt>Branch</dt>
          <dd class="review-branch">{page.branch}</dd>{/if}
        {#if review.tasks.length}
          <dt>Tasks</dt>
          <dd>
            {#each review.tasks as task (task)}
              {#if taskHref}<a class="review-task" href={taskHref(task)}>{task}</a>{:else}<span>{task}</span>{/if}
            {/each}
          </dd>
        {/if}
        <dt>Rounds</dt>
        <dd>
          <ol class="review-round-list">
            {#each page.rounds as r (r.number)}
              <li>
                <button
                  class="review-link"
                  onclick={() => {
                    onround(r.number, null);
                    tab = 'diffs';
                  }}>Round {r.number}</button
                >
                · {r.files.length}
                {r.files.length === 1 ? 'file' : 'files'} · {r.by} · {r.at}
              </li>
            {/each}
          </ol>
        </dd>
      </dl>
      <section class="review-conversation" aria-label="Comments on the review">
        <h2>Conversation</h2>
        {#each reviewChains(page.chains) as chain (chain.id)}<ChainBox {chain} {onreply} {onresolve} />{/each}
        {#if composing}
          <div class="review-composer">
            <textarea id="review-comment-body" bind:value={draft} rows="4" aria-label="Comment" {@attach focusOnShow}
            ></textarea>
            <div class="review-actions">
              <button onclick={() => (composing = false)}>Cancel</button>
              <button class="primary" disabled={!draft.trim() || busy} onclick={saveComment}>Comment</button>
            </div>
          </div>
        {:else}
          <button class="review-add-comment" onclick={() => (composing = true)}>Comment on the review</button>
        {/if}
      </section>
    </div>
  {:else if tab === 'diffs'}
    <div
      class="review-split"
      role="tabpanel"
      aria-labelledby="review-tab-diffs"
      bind:this={split}
      style:--tree="{width}px"
    >
      <nav class="review-files" aria-label="Files">
        {#if !files.length}<p class="review-empty">No files changed here.</p>{/if}
        <ul>
          {#each tree as row (row.kind === 'folder' ? `d:${row.path}` : `f:${row.file.path}`)}
            {#if row.kind === 'folder'}
              <li class="review-folder" style:--depth={row.depth}>{row.name}/</li>
            {:else}
              {@const counts = lineCounts(row.file.patch)}
              <li
                class="review-file"
                class:current={row.file.path === atFile}
                class:viewed={isViewed(row.file)}
                style:--depth={row.depth}
              >
                <input
                  type="checkbox"
                  aria-label="Viewed {row.file.path}"
                  checked={isViewed(row.file)}
                  onchange={() => toggleViewed(row.file)}
                />
                <button class="review-file-name" data-file={row.file.path} onclick={() => showFile(row.file.path)}>
                  <span class="review-file-status status-{row.file.status}">{statusLetter[row.file.status]}</span>
                  {row.name}
                </button>
                <span class="review-file-counts">
                  {#if row.file.binary}bin{:else}<span class="added">+{counts.added}</span>
                    <span class="removed">−{counts.removed}</span>{/if}
                </span>
              </li>
            {/if}
          {/each}
        </ul>
      </nav>
      <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions (a splitter is
           dragged, and moved with the arrow keys) -->
      <div
        class="review-splitter"
        role="separator"
        aria-orientation="vertical"
        aria-label="Resize the files"
        aria-valuenow={width}
        aria-valuemin={TREE_WIDTH.least}
        tabindex="0"
        onpointerdown={drag}
        onkeydown={nudge}
      ></div>
      <div class="review-diffs" bind:this={list} {onscroll}>
        {#each files as f (f.path)}
          <ReviewFile
            file={f}
            chains={chainsOf(page.chains, f)}
            drifted={driftedOf(page.chains, f)}
            before={sideOf(f.before)}
            after={sideOf(f.after)}
            viewed={isViewed(f)}
            folded={folded(f)}
            {mode}
            view={rendered}
            {busy}
            root={list}
            wanted={f.path === wanted}
            onfold={() => (folds = { ...folds, [f.path]: !folded(f) })}
            onviewed={() => toggleViewed(f)}
            oncomment={(picked, body) => oncomment({ path: f.path, picked, body })}
            {onreply}
            {onresolve}
            onround={(round) => onround(round, null)}
          />
        {/each}
      </div>
    </div>
  {:else}
    <div class="review-comments" role="tabpanel" aria-labelledby="review-tab-comments">
      <div class="review-switch review-filter" role="group" aria-label="Show">
        {#each [['all', 'All'], ['open', 'Open'], ['resolved', 'Resolved']] as const as [id, label] (id)}
          <button aria-pressed={filter === id} data-filter={id} onclick={() => (filter = id)}
            >{label} <span class="review-count">{chainCounts[id]}</span></button
          >
        {/each}
      </div>
      {#if !listed.length}
        <p class="review-empty">
          {filter === 'all' ? 'No comments yet.' : filter === 'open' ? 'Nothing open.' : 'Nothing resolved yet.'}
        </p>
      {/if}
      {#each listed as chain (chain.id)}
        <ChainBox
          chain={asMade(chain)}
          {onreply}
          {onresolve}
          onwhere={chain.anchor.path ? () => showFile(chain.anchor.path!) : () => (tab = 'overview')}
        />
      {/each}
    </div>
  {/if}
</article>

<style>
  .review-page {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    background: var(--bg);
  }
  /* The top bar: the title and Submit, then the tabs and the Diffs tab's tools */
  .review-top {
    flex: none;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
  }
  .review-top-row {
    display: flex;
    align-items: center;
    gap: var(--space-5);
    padding: var(--space-5) var(--space-7) 0;
  }
  .review-bar {
    flex-wrap: wrap;
    padding-top: var(--space-3);
    padding-bottom: 0;
  }
  .review-back {
    color: var(--text-muted);
    font-size: var(--text-sm);
    text-decoration: none;
    white-space: nowrap;
  }
  .review-back:hover {
    color: var(--accent);
  }
  .review-title {
    display: flex;
    align-items: baseline;
    gap: var(--space-4);
    min-width: 0;
    margin: 0;
    font-size: var(--text-xl);
  }
  .review-project,
  .review-ref,
  .review-branch {
    font-family: 'SF Mono', Menlo, Consolas, monospace;
    color: var(--text-muted);
    white-space: nowrap;
  }
  .review-project {
    font-weight: 400;
  }
  .review-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .review-status,
  .review-needs {
    padding: var(--space-1) var(--space-4);
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    font-size: var(--text-xs);
    white-space: nowrap;
    color: var(--text-muted);
  }
  .review-status.status-approved {
    color: var(--green);
    border-color: var(--green);
  }
  .review-status.status-changes_requested {
    color: var(--red);
    border-color: var(--red);
  }
  .review-status.status-open,
  .review-status.status-live {
    color: var(--accent);
    border-color: var(--accent);
  }
  .review-needs {
    color: var(--on-accent);
    background: var(--accent);
    border-color: var(--accent);
  }
  .review-submit-anchor {
    position: relative;
    margin-left: auto;
  }
  .review-submit {
    position: absolute;
    right: 0;
    top: calc(100% + var(--space-3));
    z-index: 10;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    width: min(26rem, 90vw);
    padding: var(--space-6);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--surface);
    box-shadow: 0 8px 24px color-mix(in srgb, var(--text) 18%, transparent);
  }
  .review-tabs {
    display: flex;
    gap: var(--space-2);
  }
  .review-tabs button {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-4) var(--space-6);
    border: none;
    border-bottom: 2px solid transparent;
    background: none;
    color: var(--text-muted);
    font: inherit;
    font-size: var(--text-md);
    cursor: pointer;
  }
  .review-tabs button[aria-selected='true'] {
    color: var(--text);
    font-weight: 600;
    border-bottom-color: var(--accent);
  }
  .review-count {
    padding: 0 var(--space-3);
    border-radius: var(--radius-pill);
    background: color-mix(in srgb, var(--text-muted) 18%, transparent);
    font-size: var(--text-xs);
    font-weight: 400;
  }
  .review-tools {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-5);
    margin-left: auto;
    padding-bottom: var(--space-3);
    font-size: var(--text-sm);
    color: var(--text-muted);
  }
  .review-tools select {
    margin-left: var(--space-2);
    padding: var(--space-1) var(--space-3);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg);
    color: var(--text);
    font: inherit;
  }
  .review-progress {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }
  .review-meter {
    width: 5rem;
    height: var(--space-3);
    border-radius: var(--radius-pill);
    background: color-mix(in srgb, var(--text-muted) 20%, transparent);
    overflow: hidden;
  }
  .review-meter span {
    display: block;
    height: 100%;
    background: var(--green);
  }
  .review-switch {
    display: flex;
  }
  .review-switch button {
    padding: var(--space-2) var(--space-4);
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text-muted);
    font: inherit;
    font-size: var(--text-xs);
    cursor: pointer;
  }
  .review-switch button + button {
    border-left: none;
  }
  .review-switch button:first-child {
    border-radius: var(--radius-sm) 0 0 var(--radius-sm);
  }
  .review-switch button:last-child {
    border-radius: 0 var(--radius-sm) var(--radius-sm) 0;
  }
  .review-switch button[aria-pressed='true'] {
    background: var(--accent);
    color: var(--on-accent);
    border-color: var(--accent);
  }
  .review-problem {
    margin: var(--space-4) var(--space-7) 0;
    color: var(--red);
  }
  /* Overview */
  .review-overview,
  .review-comments {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: var(--space-8) max(var(--space-7), calc((100% - 56rem) / 2));
  }
  .review-summary {
    padding: var(--space-6) var(--space-8);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
  }
  .review-facts {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: var(--space-3) var(--space-8);
    margin: var(--space-8) 0;
    font-size: var(--text-md);
  }
  .review-facts dt {
    color: var(--text-muted);
  }
  .review-facts dd {
    margin: 0;
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-4);
  }
  .review-round-list {
    margin: 0;
    padding: 0;
    list-style: none;
    color: var(--text-muted);
  }
  .review-conversation h2 {
    font-size: var(--text-lg);
    margin: 0 0 var(--space-4);
  }
  .review-conversation :global(.chain) {
    margin-left: 0;
    margin-right: 0;
  }
  .review-link,
  .review-add-comment {
    border: none;
    background: none;
    color: var(--accent);
    font: inherit;
    cursor: pointer;
    padding: 0;
  }
  .review-add-comment {
    margin-top: var(--space-4);
  }
  /* Diffs: the tree, the splitter, the list */
  .review-split {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: var(--tree) var(--space-3) minmax(0, 1fr);
  }
  .review-files {
    overflow: auto;
    padding: var(--space-4) 0;
    font-size: var(--text-sm);
    background: var(--surface);
  }
  .review-files ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .review-folder,
  .review-file {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-1) var(--space-4) var(--space-1) calc(var(--space-4) + var(--depth) * var(--space-6));
  }
  .review-folder {
    color: var(--text-muted);
  }
  .review-file.current {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .review-file.viewed .review-file-name {
    color: var(--text-muted);
  }
  .review-file-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    border: none;
    background: none;
    color: var(--text);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .review-file-status {
    font-family: 'SF Mono', Menlo, Consolas, monospace;
    color: var(--yellow);
  }
  .review-file-status.status-added {
    color: var(--green);
  }
  .review-file-status.status-deleted {
    color: var(--red);
  }
  .review-file-counts {
    font-size: var(--text-xs);
    color: var(--text-muted);
    white-space: nowrap;
  }
  .added {
    color: var(--green);
  }
  .removed {
    color: var(--red);
  }
  .review-splitter {
    cursor: col-resize;
    border-left: 1px solid var(--border);
    background: transparent;
    touch-action: none;
  }
  .review-splitter:hover,
  .review-splitter:focus-visible {
    background: color-mix(in srgb, var(--accent) 30%, transparent);
    outline: none;
  }
  .review-diffs {
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
    padding: var(--space-6) var(--space-7) 50vh;
  }
  .review-diffs :global(.review-current-change) {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  /* Comments */
  .review-filter {
    margin-bottom: var(--space-6);
  }
  .review-comments :global(.chain) {
    margin-left: 0;
    margin-right: 0;
  }
  .review-composer {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    margin-top: var(--space-4);
  }
  textarea {
    width: 100%;
    box-sizing: border-box;
    padding: var(--space-3);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg);
    color: var(--text);
    font: inherit;
  }
  .review-actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--space-4);
  }
  .review-actions button,
  .review-submit-toggle {
    padding: var(--space-2) var(--space-5);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg);
    color: var(--text);
    font: inherit;
    font-size: var(--text-md);
    cursor: pointer;
  }
  .review-actions button.primary,
  .review-submit-toggle.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  .review-actions button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .review-empty {
    color: var(--text-muted);
  }
  @media (max-width: 720px) {
    .review-split {
      grid-template-columns: 1fr;
      grid-template-rows: auto 1fr;
    }
    .review-splitter {
      display: none;
    }
    .review-files {
      max-height: 30vh;
    }
  }
</style>
