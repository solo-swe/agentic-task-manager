<script lang="ts">
  import type { TaskLinks } from '../compmd';
  import { formatAgoOrDate } from '../format';
  import { readStored, writeStored } from '../prefs';
  import Icon from '../ui/Icon.svelte';
  import { groupReviews, waitedFor } from './page';
  import { offerReviewSearch } from './search.svelte';
  import {
    NO_QUERY,
    STATUS_LABELS,
    type ListQuery as Query,
    type ReviewEntry,
    type ReviewStatus,
    type StatusFilter,
    type UnreadableReview,
  } from './types';

  // The reviews page (docs/code-review-design.md §3.11, ATM-574): a project's reviews, the latest activity first,
  // with search (title, summary and comments, every word) and filters (status, author, reviewer, task, branch); those
  // that need the user marked, and *Next review needing you*. The list and the query are the page's: it asks the
  // server again when the query changes (`onquery`), and opens a review (`onopen`, saying whether ⌘ or Ctrl was held; the page opens it in a tab of its own).

  let {
    entries,
    query,
    loading = false,
    problem = null,
    href,
    onquery,
    onopen,
    onlive,
    onpoke,
    taskHref,
    taskLinks,
    unreadable = [],
  }: {
    entries: ReviewEntry[];
    query: Query;
    loading?: boolean;
    problem?: string | null;
    /** A review's address, for its link */
    href: (reference: string, entry?: ReviewEntry) => string;
    onquery: (query: Query) => void;
    /** `entry`: the row's, for a list of several projects' reviews (ATM-575), where a reference names one in each */
    onopen: (reference: string, newTab: boolean, entry?: ReviewEntry) => void;
    /** A live review opened (ATM-576): why it was refused, or null once it is open */
    onlive?: (live: { title: string; base: string; task: string }) => Promise<string | null>;
    /** A row's review poked (ATM-611): a nudge its agent answers; why it wasn't, if it wasn't */
    onpoke?: (entry: ReviewEntry) => Promise<string | null>;
    /** A row's tasks (ATM-593): each key that names a task a link (`taskHref` its address, `taskLinks.open` what a
     * click does: its details here, or with ⌘, Ctrl or a middle click a tab of its own); without them, keys as text */
    taskHref?: (key: string) => string;
    taskLinks?: TaskLinks;
    /** The reviews atm can't read (ATM-617): rows of their own, with why, after the others and whatever the query */
    unreadable?: UnreadableReview[];
  } = $props();

  function openTask(e: MouseEvent, key: string) {
    if (!taskLinks) return;
    e.preventDefault();
    e.stopPropagation();
    taskLinks.open(key, e.metaKey || e.ctrlKey || e.button === 1);
  }

  // New live review: a title, the task it is about, the commit it is against (HEAD when none)
  let starting = $state(false);
  let live = $state({ title: 'Live review', base: '', task: '' });
  let refused = $state<string | null>(null);
  let opening = $state(false);
  async function startLive() {
    if (!onlive || !live.title.trim()) return;
    opening = true;
    refused = await onlive({ title: live.title.trim(), base: live.base.trim(), task: live.task.trim() });
    opening = false;
    if (!refused) {
      starting = false;
      live = { title: 'Live review', base: '', task: '' };
    }
  }

  // Open first, every review not decided yet (ATM-613), then each status to narrow it further
  const STATUSES: StatusFilter[] = ['open', ...(Object.keys(STATUS_LABELS) as ReviewStatus[])];
  const FILTER_LABELS: Record<StatusFilter, string> = { open: 'Open', ...STATUS_LABELS };

  // The choices each filter offers: what the reviews listed have, and what is chosen
  const choices = (pick: (e: ReviewEntry) => (string | null)[], chosen: string) =>
    [...new Set([...entries.flatMap(pick), chosen].filter((v): v is string => !!v))].sort();
  const authors = $derived(choices((e) => [e.review.author], query.author));
  const reviewers = $derived(choices((e) => e.review.reviewers, query.reviewer));
  const tasks = $derived(choices((e) => e.review.tasks, query.task));
  const branches = $derived(choices((e) => [e.branch], query.branch));

  const filtering = $derived(
    !!(query.q || query.status.length || query.author || query.reviewer || query.task || query.branch),
  );

  // Unfiltered, the list in groups (ATM-610): what waits for you first, the longest waiting on top; any filter or
  // search, the plain list. *Older* is folded unless opened, which this browser remembers
  const GROUPS_KEY = 'atm-review-groups';
  const groups = $derived(filtering ? null : groupReviews(entries, Date.now()));
  let olderOpen = $state(readStored(GROUPS_KEY).olderOpen === true);

  // Its decision or round, or an answer since you last wrote (ATM-606): *Next review needing you* takes both, in the
  // order shown (in groups, the longest waiting for your review first, ATM-610)
  const waiting = $derived(
    (groups ? groups.flatMap((g) => g.entries) : entries).filter((e) => e.needs_you || e.waiting === 'answered'),
  );

  // The search is the title bar's, where a project's is (ATM-738): offered while the list is shown, and what is typed
  // there is the query's text
  $effect(() =>
    offerReviewSearch({
      get q() {
        return query.q;
      },
      search: (q) => onquery({ ...query, q }),
    }),
  );

  function toggleStatus(status: StatusFilter) {
    const status_ = query.status.includes(status)
      ? query.status.filter((s) => s !== status)
      : [...query.status, status];
    onquery({ ...query, status: status_ });
  }

  /** Each row's poke (ATM-611), by review: being sent, sent, or why it wasn't */
  let poked = $state<Record<string, string>>({});
  async function poke(entry: ReviewEntry) {
    const id = entry.review.id;
    poked = { ...poked, [id]: 'sending' };
    const refused = await onpoke?.(entry);
    poked = { ...poked, [id]: refused ?? 'sent' };
  }

  function open(e: MouseEvent, entry: ReviewEntry) {
    if (e.button !== 0 || e.shiftKey || e.altKey) return;
    e.preventDefault();
    onopen(entry.review.reference, e.metaKey || e.ctrlKey, entry);
  }

  const ago = (at: string) => formatAgoOrDate(at, Date.now());
</script>

<div class="review-list" id="review-list">
  {#if waiting.length || onlive}<div class="review-list-bar">
      {#if waiting.length}
        <button class="review-next" onclick={() => onopen(waiting[0].review.reference, false, waiting[0])}>
          Next review needing you <span class="review-count">{waiting.length}</span>
        </button>
      {/if}
      {#if onlive}
        <button
          class="review-new-live"
          id="review-new-live"
          aria-expanded={starting}
          title="Review a change while it is being made: the working copy, followed as it changes"
          onclick={() => (starting = !starting)}><Icon name="eye" size={15} /> New live review</button
        >
      {/if}
    </div>{/if}
  {#if starting}
    <form
      class="review-live-form"
      aria-label="New live review"
      onsubmit={(e) => {
        e.preventDefault();
        void startLive();
      }}
    >
      <label
        ><span>Title</span><input id="review-live-title" bind:value={live.title} required aria-label="Title" /></label
      >
      <label
        ><span>Task</span><input
          id="review-live-task"
          bind:value={live.task}
          placeholder="ATM-12 (optional)"
          aria-label="Task"
        /></label
      >
      <label
        ><span>Against</span><input
          id="review-live-base"
          bind:value={live.base}
          placeholder="HEAD"
          aria-label="Against the commit"
        /></label
      >
      <div class="review-live-actions">
        <button type="button" class="review-clear" onclick={() => (starting = false)}>Cancel</button>
        <button type="submit" class="review-next" disabled={opening || !live.title.trim()}>Start</button>
      </div>
      {#if refused}<p class="review-problem" role="alert">{refused}</p>{/if}
    </form>
  {/if}
  <div class="review-filters" role="group" aria-label="Filters">
    {#each STATUSES as status (status)}
      <button
        class="review-chip status-{status}"
        aria-pressed={query.status.includes(status)}
        data-status={status}
        title={status === 'open' ? 'Every review not decided yet: draft, waiting, live or sent back' : undefined}
        onclick={() => toggleStatus(status)}>{FILTER_LABELS[status]}</button
      >
    {/each}
    {#each [['author', 'Author', authors], ['reviewer', 'Reviewer', reviewers], ['task', 'Task', tasks], ['branch', 'Branch', branches]] as const as [key, label, options] (key)}
      <select
        class="review-select"
        aria-label={label}
        data-filter={key}
        value={query[key]}
        onchange={(e) => onquery({ ...query, [key]: e.currentTarget.value })}
      >
        <option value="">Any {label.toLowerCase()}</option>
        {#each options as option (option)}<option value={option}>{option}</option>{/each}
      </select>
    {/each}
    {#if filtering}
      <button class="review-clear" onclick={() => onquery({ ...NO_QUERY })}>Clear</button>
    {/if}
  </div>

  {#if problem}
    <p class="review-problem" role="alert">{problem}</p>
  {:else if !entries.length && !unreadable.length}
    <p class="review-empty">
      {loading
        ? 'Loading…'
        : filtering
          ? 'No review fits.'
          : 'No reviews yet. Agents open them with atm review create.'}
    </p>
  {:else}
    {#if groups}
      {#each groups as group (group.id)}
        {@const folded = group.id === 'older' && !olderOpen}
        <section class="review-group" data-group={group.id} aria-label={group.title}>
          <h3 class="review-group-title">
            {#if group.id === 'older'}
              <button
                class="review-group-fold"
                aria-expanded={!folded}
                onclick={() => {
                  olderOpen = !olderOpen;
                  writeStored(GROUPS_KEY, { olderOpen });
                }}
                ><Icon name={folded ? 'chevron-right' : 'chevron-down'} size={14} />{group.title}
                <span class="review-group-count">{group.entries.length}</span></button
              >
            {:else}
              {group.title} <span class="review-group-count">{group.entries.length}</span>
            {/if}
          </h3>
          {#if !folded}
            <ul class="review-rows" aria-busy={loading}>
              {#each group.entries as entry (entry.review.id)}{@render row(entry, group.id === 'waiting')}{/each}
            </ul>
          {/if}
        </section>
      {/each}
      {#if unreadable.length}
        <ul class="review-rows">
          {#each unreadable as u (u.folder)}{@render unreadableRow(u)}{/each}
        </ul>
      {/if}
    {:else}
      <ul class="review-rows" aria-busy={loading}>
        {#each entries as entry (entry.review.id)}{@render row(entry, false)}{/each}
        {#each unreadable as u (u.folder)}{@render unreadableRow(u)}{/each}
      </ul>
    {/if}
  {/if}
</div>

{#snippet row(entry: ReviewEntry, waited: boolean)}
  {@const r = entry.review}
  <li class="review-row" class:needs-you={entry.needs_you || entry.waiting === 'answered'} data-review={r.reference}>
    <a class="review-link" href={href(r.reference, entry)} onclick={(e) => open(e, entry)}>
      <span class="review-ref">{r.reference}</span>
      <span class="review-title">{r.title}</span>
    </a>
    <span class="review-status status-{r.status}">{STATUS_LABELS[r.status]}</span>
    {#if entry.needs_you}<span class="review-needs" title="Needs you">Needs you</span>
    {:else if entry.waiting === 'answered'}<span
        class="review-needs answered"
        title="Someone wrote in it after you last did">New reply</span
      >{/if}
    {#if waited && entry.waiting_since}<span class="review-waited" title="Waiting since {entry.waiting_since}"
        >Waited {waitedFor(entry.waiting_since, Date.now())}</span
      >{/if}
    {#if onpoke && r.status !== 'approved' && r.status !== 'closed'}
      <!-- A poke (ATM-611): its agent hears it and answers, saying where the review stands -->
      <button
        class="review-poke"
        data-review={r.reference}
        disabled={poked[r.id] === 'sending'}
        title={poked[r.id] && poked[r.id] !== 'sent' && poked[r.id] !== 'sending'
          ? `Not poked: ${poked[r.id]}`
          : 'Nudge its agent: it answers, saying where the review stands'}
        onclick={() => void poke(entry)}
        ><Icon name="bell" size={12} />{poked[r.id] === 'sent' ? 'Poked' : 'Poke'}</button
      >
    {/if}
    <span class="review-meta">
      {#if entry.project}<span class="review-project" data-project={entry.project.id}
          ><Icon name="folder" size={12} />{entry.project.name}</span
        >{/if}
      {#if r.tasks.length}<span class="review-tasks"
          ><Icon name="list-todo" size={12} />{#each r.tasks as key, i (key)}{i
              ? ', '
              : ''}{#if taskHref && taskLinks?.known(key)}<a
                class="review-task-link"
                href={taskHref(key)}
                data-task={key}
                onclick={(e) => openTask(e, key)}
                onauxclick={(e) => e.button === 1 && openTask(e, key)}>{key}</a
              >{:else}{key}{/if}{/each}</span
        >{/if}
      {#if entry.branch}<span class="review-branch"><Icon name="git-branch" size={12} />{entry.branch}</span>{/if}
      <span><Icon name="user" size={12} />{r.author ?? 'no author yet'}</span>
      <span title="Open chains of comments"
        ><Icon name="message-square" size={12} />{entry.open} open of {entry.chains}</span
      >
      <span title={r.updated}><Icon name="history" size={12} />{ago(r.updated)}</span>
    </span>
  </li>
{/snippet}

{#snippet unreadableRow(u: UnreadableReview)}
  <li class="review-row review-unreadable" data-unreadable={u.reference}>
    <span class="review-ref">{u.reference}</span>
    <span class="review-status status-unreadable">Can't be read</span>
    <span class="review-meta">
      <span class="review-unreadable-why" title={u.folder}>{u.problem}</span>
      <span>A newer atm may have written it.</span>
    </span>
  </li>
{/snippet}

<style>
  .review-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
    padding: var(--space-7) var(--space-6);
  }
  .review-list-bar,
  .review-filters {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-4);
  }
  .review-next,
  .review-clear,
  .review-chip,
  .review-select {
    height: 28px;
    padding: 0 var(--space-5);
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: var(--text-sm);
    cursor: pointer;
  }
  .review-select {
    border-radius: var(--radius-md);
  }
  .review-new-live {
    display: inline-flex;
    align-items: center;
    gap: var(--space-3);
    height: 34px;
    padding: 0 var(--space-6);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: var(--text-sm);
    cursor: pointer;
  }
  .review-new-live:hover {
    background: var(--surface2);
  }
  .review-live-form {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: var(--space-5);
    padding: var(--space-5);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--surface);
  }
  .review-live-form label {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    font-size: var(--text-xs);
    color: var(--text-muted);
  }
  .review-live-form input {
    height: 28px;
    padding: 0 var(--space-4);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg);
    color: var(--text);
    font: inherit;
    font-size: var(--text-sm);
  }
  .review-live-actions {
    display: flex;
    gap: var(--space-3);
    margin-left: auto;
  }
  .review-chip:hover,
  .review-clear:hover {
    background: var(--surface2);
  }
  .review-next {
    display: inline-flex;
    align-items: center;
    height: 34px;
    padding: 0 var(--space-6);
    border-radius: var(--radius-md);
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
    font-weight: 500;
  }
  .review-count {
    margin-left: var(--space-3);
    padding: 0 var(--space-3);
    border-radius: var(--radius-pill);
    background: color-mix(in srgb, var(--on-accent) 20%, transparent);
    font-weight: 600;
  }
  .review-chip[aria-pressed='true'] {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    border-color: var(--accent);
    color: var(--accent);
    font-weight: 600;
  }
  .review-rows {
    list-style: none;
    margin: 0;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--surface);
    overflow: hidden;
  }
  .review-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2) var(--space-4);
    padding: var(--space-6) var(--space-7);
    border-top: 1px solid var(--border);
  }
  .review-row:hover {
    background: var(--surface2);
  }
  .review-row:first-child {
    border-top: none;
  }
  .review-row.needs-you {
    box-shadow: inset 3px 0 0 var(--purple);
  }
  .review-link {
    display: flex;
    gap: var(--space-3);
    flex: 1;
    min-width: 12rem;
    color: var(--text);
    text-decoration: none;
  }
  .review-link:hover .review-title {
    text-decoration: underline;
  }
  .review-ref {
    font-family: 'SF Mono', Menlo, Consolas, monospace;
    color: var(--text-muted);
  }
  .review-title {
    font-size: var(--text-lg);
    font-weight: 600;
  }
  .review-status,
  .review-needs {
    padding: var(--space-1) var(--space-5);
    border-radius: var(--radius-pill);
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--text-muted);
    border: 1px solid var(--border);
  }
  .review-status.status-approved {
    color: var(--green);
    border-color: color-mix(in srgb, var(--green) 50%, transparent);
    background: color-mix(in srgb, var(--green) 10%, transparent);
  }
  .review-status.status-changes_requested {
    color: var(--red);
    border-color: color-mix(in srgb, var(--red) 50%, transparent);
    background: color-mix(in srgb, var(--red) 10%, transparent);
  }
  .review-status.status-unreadable {
    color: var(--red);
    border-color: color-mix(in srgb, var(--red) 50%, transparent);
  }
  .review-unreadable-why {
    color: var(--red);
  }
  .review-status.status-waiting,
  .review-status.status-live {
    color: var(--accent);
    border-color: color-mix(in srgb, var(--accent) 50%, transparent);
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }
  .review-needs {
    color: var(--purple);
    border-color: color-mix(in srgb, var(--purple) 50%, transparent);
    background: color-mix(in srgb, var(--purple) 10%, transparent);
  }
  .review-task-link {
    color: var(--accent);
    text-decoration: none;
  }
  .review-task-link:hover {
    text-decoration: underline;
  }
  .review-meta {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-7);
    width: 100%;
    color: var(--text-muted);
    font-size: var(--text-xs);
  }
  .review-meta > span {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
  }
  .review-branch {
    font-family: 'SF Mono', Menlo, Consolas, monospace;
  }
  .review-empty,
  .review-problem {
    color: var(--text-muted);
  }
  .review-group {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }
  .review-group-title {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin: 0;
    color: var(--text-muted);
    font-size: var(--text-sm);
    font-weight: 600;
  }
  .review-group-fold {
    display: inline-flex;
    align-items: center;
    gap: var(--space-3);
    padding: 0;
    border: none;
    background: none;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .review-group-count {
    padding: 0 var(--space-3);
    border-radius: var(--radius-pill);
    background: var(--surface2);
    font-weight: 600;
  }
  .review-poke {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-1) var(--space-4);
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    background: var(--surface);
    color: var(--text-muted);
    font: inherit;
    font-size: var(--text-xs);
    cursor: pointer;
  }
  .review-poke:hover:not(:disabled) {
    color: var(--purple);
    border-color: color-mix(in srgb, var(--purple) 50%, transparent);
  }
  .review-waited {
    color: var(--purple);
    font-size: var(--text-xs);
    font-weight: 600;
  }
</style>
