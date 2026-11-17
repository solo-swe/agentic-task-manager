<script lang="ts" module>
  /** How far past the view a card is made ahead of being scrolled to, in pixels */
  export const MARGIN = 800;

  /** Whether an entry of an observer with MARGIN is in the view itself (its root's box without the margin) */
  export function inView(entry: IntersectionObserverEntry): boolean {
    const view = entry.rootBounds;
    if (!view) return true;
    const box = entry.boundingClientRect;
    return box.bottom > view.top + MARGIN && box.top < view.bottom - MARGIN;
  }

  /** Run `act` once the page is idle (or after a moment, where there is no idle callback); the canceller back */
  export function whenIdle(act: () => void): () => void {
    if (typeof requestIdleCallback === 'function') {
      const id = requestIdleCallback(act, { timeout: 1000 });
      return () => cancelIdleCallback(id);
    }
    const id = setTimeout(act, 150);
    return () => clearTimeout(id);
  }

  /** Until when the cards only near the view wait, after a jump: the file gone to is drawn first */
  let quietUntil = 0;
  /** A jump to a file: the other cards, in the view or near it, wait `ms` before they are made (the file gone to is
   * made at once: `wanted`) */
  export function holdNearCards(ms = 400) {
    quietUntil = performance.now() + ms;
  }
  const holding = () => performance.now() < quietUntil;

  /** `act` once the page is idle and no jump holds the cards back; the canceller back */
  export function whenSettled(act: () => void): () => void {
    let cancel = () => {};
    const attempt = () => {
      const wait = quietUntil - performance.now();
      if (wait <= 0) return act();
      const id = setTimeout(() => (cancel = whenIdle(attempt)), wait);
      cancel = () => clearTimeout(id);
    };
    cancel = whenIdle(attempt);
    return () => cancel();
  }
</script>

<script lang="ts">
  import type { DocumentSource } from '../documents/source';
  import DiffView from './DiffView.svelte';
  import { lineCounts } from './page';
  import type { Chain, PlacedChain, Picked, ViewFile } from './types';

  // A file of a review's Diffs tab (ATM-574), as a card in the list of the round's files: its header (fold, status,
  // path, what it adds and removes, Viewed) and its diff. A file marked viewed folds; the fold opens it again. Its
  // diff is made once the card nears the view (a review of hundreds of files makes only what is looked at), and kept.
  let {
    file,
    chains,
    drifted,
    before,
    after,
    viewed,
    folded,
    mode,
    view,
    busy = false,
    root = null,
    wanted = false,
    onfold,
    onviewed,
    oncomment,
    onreply,
    onresolve,
    onround,
  }: {
    file: ViewFile;
    chains: Chain[];
    drifted: PlacedChain[];
    before: DocumentSource | null;
    after: DocumentSource | null;
    viewed: boolean;
    folded: boolean;
    mode: 'split' | 'unified';
    view: 'source' | 'rendered';
    busy?: boolean;
    /** The list that scrolls, which the card is made in view of */
    root?: HTMLElement | null;
    /** The file just gone to: made at once */
    wanted?: boolean;
    onfold: () => void;
    onviewed: () => void;
    oncomment: (picked: Picked, body: string) => void;
    onreply: (to: string, body: string) => void;
    onresolve: (comment: string, resolved: boolean) => void;
    onround: (round: number) => void;
  } = $props();

  const counts = $derived(lineCounts(file.patch));
  const statusLetter = { added: 'A', modified: 'M', deleted: 'D', renamed: 'R' } as const;

  // A card in view is made at once; one only near it (within MARGIN), once the page is idle. Right after a jump, only
  // the file gone to (`wanted`) is made at once, the others a moment later: its sides load first
  let el = $state<HTMLElement>();
  let near = $state(typeof IntersectionObserver === 'undefined');
  $effect(() => {
    if (wanted) near = true;
  });
  $effect(() => {
    const target = el;
    if (!target || near) return;
    let later: (() => void) | null = null;
    const seen = new IntersectionObserver(
      (entries) => {
        const entry = entries.find((e) => e.isIntersecting);
        if (!entry) return;
        if (inView(entry) && !holding()) {
          later?.();
          near = true;
        } else later ??= whenSettled(() => (near = true));
      },
      { root, rootMargin: `${MARGIN}px 0px` },
    );
    seen.observe(target);
    return () => {
      seen.disconnect();
      later?.();
    };
  });
  /** How tall a diff not made yet stands: about its lines, so the list's scroll bar is about right */
  const estimate = $derived(Math.min(4000, (counts.added + counts.removed + 8) * 20));
</script>

<section
  class="review-file-card"
  class:folded
  bind:this={el}
  data-file={file.path}
  data-pending={!folded && !near && !file.binary ? '' : undefined}
  aria-label={file.path}
>
  <header class="review-file-head">
    <button
      class="review-fold"
      aria-expanded={!folded}
      aria-label="{folded ? 'Show' : 'Hide'} {file.path}"
      onclick={onfold}>{folded ? '▸' : '▾'}</button
    >
    <span class="review-file-status status-{file.status}" title={file.status}>{statusLetter[file.status]}</span>
    <span class="review-file-path">{file.old_path !== file.path ? `${file.old_path} → ${file.path}` : file.path}</span>
    <span class="review-file-counts">
      {#if file.binary}binary{:else}<span class="added">+{counts.added}</span>
        <span class="removed">−{counts.removed}</span>{/if}
    </span>
    {#if chains.length}<span class="review-file-chains"
        >{chains.length} {chains.length === 1 ? 'comment' : 'comments'}</span
      >{/if}
    <label class="review-file-viewed" class:on={viewed}>
      <input type="checkbox" aria-label="Viewed {file.path}" checked={viewed} onchange={onviewed} /> Viewed
    </label>
  </header>
  {#if !folded}
    <div class="review-file-body">
      {#each drifted as chain (chain.id)}
        <p class="review-drifted" data-chain={chain.id}>
          A comment's text is gone from this round:
          <button class="review-drifted-link" onclick={() => chain.drifted && onround(chain.drifted.round)}>
            see it where it was made, round {chain.drifted?.round}{chain.drifted?.lines
              ? `, lines ${chain.drifted.lines[0]}–${chain.drifted.lines[1]}`
              : ''}
          </button>
        </p>
      {/each}
      {#if file.binary}
        <p class="review-empty">A binary file: listed, not shown.</p>
      {:else if near}
        <DiffView
          path={file.path}
          oldPath={file.old_path}
          {before}
          {after}
          patch={file.patch}
          {chains}
          {mode}
          {view}
          {viewed}
          {busy}
          header={false}
          keyboard={false}
          {oncomment}
          {onreply}
          {onresolve}
          {onviewed}
        />
      {:else}
        <div class="review-file-later" style:height="{estimate}px"></div>
      {/if}
    </div>
  {/if}
</section>

<style>
  .review-file-card {
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg);
    scroll-margin-top: var(--space-4);
  }
  .review-file-head {
    position: sticky;
    top: 0;
    z-index: 2;
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-3) var(--space-6);
    border-bottom: 1px solid var(--border);
    border-radius: var(--radius-md) var(--radius-md) 0 0;
    background: var(--surface);
    font-size: var(--text-md);
  }
  .folded .review-file-head {
    border-bottom: none;
    border-radius: var(--radius-md);
  }
  .review-fold {
    width: var(--space-8);
    padding: 0;
    border: none;
    background: none;
    color: var(--text-muted);
    font: inherit;
    cursor: pointer;
  }
  .review-file-status {
    font-family: 'SF Mono', Menlo, Consolas, monospace;
    font-size: var(--text-xs);
    font-weight: 700;
    color: var(--yellow);
  }
  .review-file-status.status-added {
    color: var(--green);
  }
  .review-file-status.status-deleted {
    color: var(--red);
  }
  .review-file-status.status-renamed {
    color: var(--accent);
  }
  .review-file-path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: 'SF Mono', Menlo, Consolas, monospace;
    font-weight: 600;
  }
  .review-file-counts,
  .review-file-chains {
    font-size: var(--text-xs);
    color: var(--text-muted);
  }
  .added {
    color: var(--green);
  }
  .removed {
    color: var(--red);
  }
  .review-file-viewed {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-1) var(--space-4);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    font-size: var(--text-xs);
    color: var(--text-muted);
    cursor: pointer;
  }
  .review-file-viewed.on {
    border-color: var(--green);
    color: var(--green);
  }
  .review-file-body {
    overflow: hidden;
    border-radius: 0 0 var(--radius-md) var(--radius-md);
  }
  .review-drifted {
    margin: var(--space-4) var(--space-6);
    padding: var(--space-3) var(--space-5);
    border: 1px dashed var(--border);
    border-radius: var(--radius-md);
    color: var(--text-muted);
    font-size: var(--text-sm);
  }
  .review-drifted-link {
    border: none;
    background: none;
    color: var(--accent);
    font: inherit;
    cursor: pointer;
    padding: 0;
  }
  .review-empty {
    padding: var(--space-6);
    color: var(--text-muted);
  }
</style>
