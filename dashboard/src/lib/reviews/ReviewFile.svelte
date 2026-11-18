<script lang="ts" module>
  /** How far past the view a card is made ahead of being scrolled to, in pixels */
  export const MARGIN = 800;
  /** How far past the view a made card keeps its diff before giving it back (ATM-587), in pixels: well past MARGIN, so
   * a card near the edge isn't made and given back as the view moves a little */
  export const FAR = 3200;

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
  import Icon from '../ui/Icon.svelte';
  import ChainBox from './ChainBox.svelte';
  import DiffView from './DiffView.svelte';
  import { focusOnShow } from './focus';
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
    docStyle = null,
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
    /** The document style picked in Settings, for a Markdown file's rendered view */
    docStyle?: string | null;
    /** The list that scrolls, which the card is made in view of */
    root?: HTMLElement | null;
    /** The file just gone to: made at once */
    wanted?: boolean;
    onfold: () => void;
    onviewed: () => void;
    /** A comment on lines (`picked`), or on the file as a whole (none: a submodule's move) */
    oncomment: (picked: Picked | null, body: string) => Promise<string | null> | void;
    onreply: (to: string, body: string) => void;
    onresolve: (comment: string, resolved: boolean) => void;
    onround: (round: number) => void;
  } = $props();

  const counts = $derived(lineCounts(file.patch));
  const statusLetter = { added: 'A', modified: 'M', deleted: 'D', renamed: 'R' } as const;

  // A submodule's move (ATM-583): its two commits, linked to its repository when it has a web address; comments on it
  // are on the entry as a whole
  const moved = $derived(file.submodule ?? null);
  const short = (sha: string) => sha.slice(0, 7);
  const commitUrl = (sha: string) => (moved?.repository ? `${moved.repository}/commit/${sha}` : null);
  let writing = $state(false);
  let draft = $state('');
  let problem = $state<string | null>(null);
  async function save() {
    const body = draft.trim();
    if (!body) return;
    problem = (await oncomment(null, body)) ?? null;
    if (!problem) {
      draft = '';
      writing = false;
    }
  }

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
  // A card made and then far from the view gives back its diff (ATM-587: the page holds the rows near the view, not
  // every file's), standing in at the height it had, and is made again when it comes near; not while a comment is
  // being written in it, nor the file gone to
  let body = $state<HTMLElement>();
  /** The height its diff had, when made: what it stands at once given back */
  let measured = $state(0);
  $effect(() => {
    const target = el;
    if (!target || !near || wanted || typeof IntersectionObserver === 'undefined') return;
    const far = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) return;
        const writingHere = target.querySelector('textarea, [contenteditable="true"]') !== null;
        if (writingHere || target.contains(document.activeElement)) return;
        measured = body?.offsetHeight ?? measured;
        near = false;
      },
      { root, rootMargin: `${FAR}px 0px` },
    );
    far.observe(target);
    return () => far.disconnect();
  });
  /** How tall a diff not made yet stands: the height it had, else about its lines, so the scroll bar is about right */
  // No cap (ATM-587, CompMD's CR-1): a 600-line file stood at 4,000 px and grew to 12,000 once drawn, so the end of the
  // review moved away from a reader who had just scrolled to it
  const estimate = $derived(measured || (counts.added + counts.removed + 8) * 20);
</script>

<section
  class="review-file-card"
  class:folded
  bind:this={el}
  data-file={file.path}
  data-pending={!folded && !near && !file.binary && !moved ? '' : undefined}
  data-comments={chains.length ? '' : undefined}
  aria-label={file.path}
>
  <header class="review-file-head">
    <button
      class="review-fold"
      aria-expanded={!folded}
      aria-label="{folded ? 'Show' : 'Hide'} {file.path}"
      onclick={onfold}><Icon name={folded ? 'chevron-right' : 'chevron-down'} size={16} /></button
    >
    <span class="review-file-status status-{file.status}" title={file.status}>{statusLetter[file.status]}</span>
    <span class="review-file-path">{file.old_path !== file.path ? `${file.old_path} → ${file.path}` : file.path}</span>
    <span class="review-file-counts">
      {#if moved}submodule{:else if file.binary}binary{:else}<span class="added">+{counts.added}</span>
        <span class="removed">−{counts.removed}</span>{/if}
    </span>
    {#if chains.length}<span class="review-file-chains"
        >{chains.length} {chains.length === 1 ? 'comment' : 'comments'}</span
      >{/if}
    <label class="review-file-viewed" class:on={viewed}>
      <input type="checkbox" aria-label="Viewed {file.path}" checked={viewed} onchange={onviewed} />
      {#if viewed}<Icon name="check" size={13} />{/if} Viewed
    </label>
  </header>
  {#if !folded}
    <div class="review-file-body" bind:this={body}>
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
      {#if moved}
        <div class="review-submodule">
          <p class="review-submodule-move">
            <Icon name="git-commit" size={14} />
            {#snippet commit(sha: string)}
              {@const url = commitUrl(sha)}
              {#if url}<a href={url} target="_blank" rel="noopener noreferrer"><code>{short(sha)}</code></a>{:else}<code
                  >{short(sha)}</code
                >{/if}
            {/snippet}
            {#if moved.from && moved.to}
              The submodule moved from {@render commit(moved.from)} to {@render commit(moved.to)}.
            {:else if moved.to}
              The submodule was added, at {@render commit(moved.to)}.
            {:else if moved.from}
              The submodule was removed; it was at {@render commit(moved.from)}.
            {/if}
          </p>
          {#each chains as chain (chain.id)}
            <ChainBox {chain} {onreply} {onresolve} />
          {/each}
          {#if writing}
            <div class="review-submodule-composer">
              <textarea
                aria-label="Comment on {file.path}"
                bind:value={draft}
                {@attach focusOnShow}
                onkeydown={(e) => {
                  if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) void save();
                  if (e.key === 'Escape') writing = false;
                }}></textarea>
              {#if problem}<p class="review-submodule-problem" role="alert">Not saved: {problem}</p>{/if}
              <div class="review-submodule-actions">
                <button onclick={() => (writing = false)}>Cancel</button>
                <button class="primary" disabled={busy || !draft.trim()} onclick={() => void save()}>Comment</button>
              </div>
            </div>
          {:else}
            <button class="review-submodule-comment" onclick={() => (writing = true)}>
              <Icon name="message-square" size={13} /> Comment on this submodule
            </button>
          {/if}
        </div>
      {:else if file.binary}
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
          {docStyle}
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
    /* Off screen, the browser skips its layout and painting (ATM-587), standing in at the size it last had */
    content-visibility: auto;
    contain-intrinsic-size: auto 400px;
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
  .review-submodule {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-5) var(--space-6);
  }
  .review-submodule-move {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--space-2);
    margin: 0;
    color: var(--text-muted);
    font-size: var(--text-sm);
  }
  .review-submodule-move code {
    font-family: 'SF Mono', Menlo, Consolas, monospace;
    color: var(--text);
  }
  .review-submodule-move a {
    color: var(--accent);
  }
  .review-submodule-comment {
    align-self: flex-start;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-1) var(--space-4);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg);
    color: var(--text-muted);
    font: inherit;
    font-size: var(--text-xs);
    cursor: pointer;
  }
  .review-submodule-composer {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .review-submodule-composer textarea {
    min-height: calc(var(--space-8) * 3);
    padding: var(--space-3);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg);
    color: var(--text);
    font: inherit;
    font-size: var(--text-sm);
  }
  .review-submodule-actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-3);
  }
  .review-submodule-actions button {
    padding: var(--space-1) var(--space-5);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: var(--text-xs);
    cursor: pointer;
  }
  .review-submodule-actions button.primary {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--bg);
  }
  .review-submodule-problem {
    margin: 0;
    color: var(--red);
    font-size: var(--text-xs);
  }
</style>
