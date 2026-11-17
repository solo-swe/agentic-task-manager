<script lang="ts">
  import CompMd from '../documents/CompMd.svelte';
  import type { DocumentSource } from '../documents/source';
  import ChainBox from './ChainBox.svelte';
  import DiffView from './DiffView.svelte';
  import { chainsOf, driftedOf, fileTree, lineCounts, reviewChains } from './page';
  import { STATUS_LABELS, type Picked, type ReviewPageData, type ViewFile } from './types';

  // A review's page (docs/code-review-design.md §3.11, ATM-574): the header (title, status, tasks, branch, rounds, the
  // summary), the round picker (the whole change of a round, or a round against an earlier one), the files as a tree
  // with each one's changes and *viewed* tick, the diff viewer for the file in view with its chains in place, the
  // drifted chains linking to where they were made, the review's own chains, and *Submit*. Everything it does is the
  // page's: the handlers. Which file, round and view are shown is the address's (`file`, `onfile`, `onround`).
  let {
    page,
    file = null,
    side,
    isViewed,
    taskHref,
    busy = false,
    problem = null,
    onfile,
    onround,
    onviewed,
    oncomment,
    onreply,
    onresolve,
    onsubmit,
  }: {
    page: ReviewPageData;
    /** The file in view (its path), the first when none */
    file?: string | null;
    /** A side's files: the commit or tree a round's side is read from */
    side: (revision: string) => DocumentSource;
    isViewed: (file: ViewFile) => boolean;
    /** A task's address, for its link */
    taskHref?: (key: string) => string;
    busy?: boolean;
    problem?: string | null;
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
  const shown = $derived(files.find((f) => f.path === file) ?? files[0] ?? null);
  const index = $derived(shown ? files.indexOf(shown) : -1);
  const tree = $derived(fileTree(files));
  const ended = $derived(review.status === 'approved' || review.status === 'closed');
  const rounds = $derived(page.rounds.map((r) => r.number));

  function step(by: number) {
    const next = files[index + by];
    if (next) onfile(next.path);
  }

  // A chain to start: on the lines picked in the diff, or on the review as a whole
  let composing = $state<{ path: string | null; picked: Picked | null } | null>(null);
  let draft = $state('');
  function startComment(picked: Picked | null) {
    composing = { path: picked && shown ? shown.path : null, picked };
    draft = '';
  }
  function saveComment() {
    const body = draft.trim();
    if (!body || !composing) return;
    oncomment({ ...composing, body });
    composing = null;
    draft = '';
  }

  let summary = $state('');
  function submit(decision: 'approve' | 'request_changes' | 'comment') {
    onsubmit(decision, summary.trim());
  }
  // A new review or a decision made: the summary box starts empty
  $effect(() => {
    void review.updated;
    summary = '';
  });

  const viewedCount = $derived(files.filter((f) => isViewed(f)).length);
  const sideOf = (revision: string | null) => (revision ? side(revision) : null);
  const statusLetter = { added: 'A', modified: 'M', deleted: 'D', renamed: 'R' } as const;
</script>

<article class="review-page" id="review-page" data-review={review.reference} aria-busy={busy}>
  <header class="review-header">
    <h1 class="review-title">
      <span class="review-ref">{review.reference}</span>
      {review.title}
    </h1>
    <div class="review-facts">
      <span class="review-status status-{review.status}" data-status={review.status}
        >{STATUS_LABELS[review.status]}</span
      >
      {#if page.needs_you}<span class="review-needs">Needs you</span>{/if}
      <span>by {review.author ?? 'nobody yet'}</span>
      <span>
        {review.reviewers.length ? `decided by ${review.reviewers.join(', ')}` : 'decided by anyone but its author'}
      </span>
      {#if page.branch}<span class="review-branch">{page.branch}</span>{/if}
      {#each review.tasks as task (task)}
        {#if taskHref}<a class="review-task" href={taskHref(task)}>{task}</a>{:else}<span>{task}</span>{/if}
      {/each}
    </div>
    {#if page.summary.trim()}
      <div class="review-summary"><CompMd text={page.summary} /></div>
    {/if}
  </header>

  {#if problem}<p class="review-problem" role="alert">{problem}</p>{/if}

  <div class="review-rounds" role="group" aria-label="Rounds">
    <label>
      Round
      <select id="review-round" value={view.round} onchange={(e) => onround(Number(e.currentTarget.value), null)}>
        {#each rounds as n (n)}<option value={n}>{n} of {rounds.length}</option>{/each}
      </select>
    </label>
    <label>
      Showing
      <select
        id="review-against"
        value={view.against ?? ''}
        onchange={(e) => onround(view.round, e.currentTarget.value ? Number(e.currentTarget.value) : null)}
      >
        <option value="">the whole change</option>
        {#each rounds.filter((n) => n < view.round) as n (n)}<option value={n}>what changed since round {n}</option
          >{/each}
      </select>
    </label>
    <span class="review-viewed-count">{viewedCount} of {files.length} viewed</span>
  </div>

  <div class="review-body">
    <nav class="review-files" aria-label="Files">
      {#if !files.length}<p class="review-empty">No files changed here.</p>{/if}
      <ul>
        {#each tree as row (row.kind === 'folder' ? `d:${row.path}` : `f:${row.file.path}`)}
          {#if row.kind === 'folder'}
            <li class="review-folder" style:--depth={row.depth}>{row.name}/</li>
          {:else}
            {@const counts = lineCounts(row.file.patch)}
            <li class="review-file" class:current={row.file === shown} style:--depth={row.depth}>
              <input
                type="checkbox"
                aria-label="Viewed {row.file.path}"
                checked={isViewed(row.file)}
                onchange={() => onviewed(row.file)}
              />
              <button class="review-file-name" data-file={row.file.path} onclick={() => onfile(row.file.path)}>
                <span class="review-file-status status-{row.file.status}">{statusLetter[row.file.status]}</span>
                {row.name}
              </button>
              <span class="review-file-counts">
                {#if row.file.binary}binary{:else}<span class="added">+{counts.added}</span>
                  <span class="removed">−{counts.removed}</span>{/if}
              </span>
            </li>
          {/if}
        {/each}
      </ul>
    </nav>

    <div class="review-diff">
      {#if shown}
        {#each driftedOf(page.chains, shown) as chain (chain.id)}
          <p class="review-drifted" data-chain={chain.id}>
            A comment's text is gone from this round:
            <button class="review-drifted-link" onclick={() => chain.drifted && onround(chain.drifted.round, null)}>
              see it where it was made, round {chain.drifted?.round}{chain.drifted?.lines
                ? `, lines ${chain.drifted.lines[0]}–${chain.drifted.lines[1]}`
                : ''}
            </button>
          </p>
        {/each}
        {#if shown.binary}
          <p class="review-empty">{shown.path} is a binary file: listed, not shown.</p>
        {:else}
          <DiffView
            path={shown.path}
            oldPath={shown.old_path}
            before={sideOf(shown.before)}
            after={sideOf(shown.after)}
            patch={shown.patch}
            chains={chainsOf(page.chains, shown)}
            viewed={isViewed(shown)}
            oncomment={(picked) => startComment(picked)}
            {onreply}
            {onresolve}
            onviewed={() => onviewed(shown)}
            onnextfile={() => step(1)}
            onprevfile={() => step(-1)}
          />
        {/if}
      {/if}

      {#if composing}
        <div class="review-composer">
          <p class="review-composer-where">
            {#if composing.picked && composing.path}
              {composing.path}, {composing.picked.lines[0] === composing.picked.lines[1]
                ? `line ${composing.picked.lines[0]}`
                : `lines ${composing.picked.lines[0]}–${composing.picked.lines[1]}`}{composing.picked.side === 'old'
                ? ' (before)'
                : ''}
            {:else}
              The review as a whole
            {/if}
          </p>
          <!-- svelte-ignore a11y_autofocus (the user just asked to write a comment) -->
          <textarea id="review-comment-body" bind:value={draft} rows="4" aria-label="Comment" autofocus></textarea>
          <div class="review-actions">
            <button class="primary" disabled={!draft.trim() || busy} onclick={saveComment}>Comment</button>
            <button onclick={() => (composing = null)}>Cancel</button>
          </div>
        </div>
      {/if}

      <section class="review-conversation" aria-label="Comments on the review">
        {#each reviewChains(page.chains) as chain (chain.id)}<ChainBox {chain} {onreply} {onresolve} />{/each}
        {#if !composing}
          <button class="review-add-comment" onclick={() => startComment(null)}>Comment on the review</button>
        {/if}
      </section>

      {#if !ended}
        <section class="review-submit" aria-label="Submit">
          <textarea
            id="review-summary-body"
            bind:value={summary}
            rows="3"
            placeholder="A summary of your review (needed to comment)"
            aria-label="Summary"></textarea>
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
  </div>
</article>

<style>
  .review-page {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    padding: var(--space-6);
  }
  .review-title {
    margin: 0;
    font-size: var(--text-xl);
  }
  .review-ref,
  .review-branch {
    font-family: 'SF Mono', Menlo, Consolas, monospace;
    color: var(--text-muted);
  }
  .review-facts,
  .review-rounds,
  .review-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-4);
    color: var(--text-muted);
    font-size: var(--text-sm);
  }
  .review-facts {
    margin-top: var(--space-3);
  }
  .review-status,
  .review-needs {
    padding: 0 var(--space-3);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    font-size: var(--text-xs);
  }
  .review-status.status-approved {
    color: var(--green);
  }
  .review-status.status-changes_requested {
    color: var(--red);
  }
  .review-needs {
    color: var(--accent);
    border-color: var(--accent);
  }
  .review-summary {
    margin-top: var(--space-4);
  }
  .review-rounds select {
    margin-left: var(--space-2);
    font: inherit;
  }
  .review-viewed-count {
    margin-left: auto;
  }
  .review-body {
    display: grid;
    grid-template-columns: minmax(12rem, 18rem) minmax(0, 1fr);
    gap: var(--space-6);
    align-items: start;
  }
  .review-files {
    position: sticky;
    top: 0;
    max-height: calc(100vh - 8rem);
    overflow: auto;
    font-size: var(--text-sm);
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
    padding: var(--space-1) 0 var(--space-1) calc(var(--depth) * var(--space-6));
  }
  .review-folder {
    color: var(--text-muted);
  }
  .review-file.current {
    background: color-mix(in srgb, var(--accent) 10%, transparent);
    border-radius: var(--radius-sm);
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
    color: var(--text-muted);
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
  }
  .added {
    color: var(--green);
  }
  .removed {
    color: var(--red);
  }
  .review-diff {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    min-width: 0;
  }
  .review-drifted {
    margin: 0;
    padding: var(--space-3) var(--space-5);
    border: 1px dashed var(--border);
    border-radius: var(--radius-md);
    color: var(--text-muted);
    font-size: var(--text-sm);
  }
  .review-drifted-link,
  .review-add-comment {
    border: none;
    background: none;
    color: var(--accent);
    font: inherit;
    cursor: pointer;
    padding: 0;
  }
  .review-composer,
  .review-submit {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding: var(--space-5);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--surface);
  }
  .review-composer-where {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--text-muted);
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
  .review-actions button {
    padding: var(--space-2) var(--space-5);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg);
    color: var(--text);
    font: inherit;
    cursor: pointer;
  }
  .review-actions button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  .review-actions button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .review-empty,
  .review-problem {
    color: var(--text-muted);
  }
  .review-problem {
    color: var(--red);
  }
  @media (max-width: 720px) {
    .review-body {
      grid-template-columns: 1fr;
    }
    .review-files {
      position: static;
      max-height: none;
    }
  }
</style>
