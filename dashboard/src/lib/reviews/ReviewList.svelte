<script lang="ts">
  import { NO_QUERY, STATUS_LABELS, type ListQuery as Query, type ReviewEntry, type ReviewStatus } from './types';

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
  }: {
    entries: ReviewEntry[];
    query: Query;
    loading?: boolean;
    problem?: string | null;
    /** A review's address, for its link */
    href: (reference: string) => string;
    onquery: (query: Query) => void;
    onopen: (reference: string, newTab: boolean) => void;
  } = $props();

  const STATUSES = Object.keys(STATUS_LABELS) as ReviewStatus[];

  // The choices each filter offers: what the reviews listed have, and what is chosen
  const choices = (pick: (e: ReviewEntry) => (string | null)[], chosen: string) =>
    [...new Set([...entries.flatMap(pick), chosen].filter((v): v is string => !!v))].sort();
  const authors = $derived(choices((e) => [e.review.author], query.author));
  const reviewers = $derived(choices((e) => e.review.reviewers, query.reviewer));
  const tasks = $derived(choices((e) => e.review.tasks, query.task));
  const branches = $derived(choices((e) => [e.branch], query.branch));

  const waiting = $derived(entries.filter((e) => e.needs_you));

  // What is typed, until the query takes it (a moment later), then the query's
  let search = $derived(query.q);
  let timer: ReturnType<typeof setTimeout> | undefined;
  function typed(text: string) {
    search = text;
    clearTimeout(timer);
    timer = setTimeout(() => onquery({ ...query, q: text.trim() }), 250);
  }

  function toggleStatus(status: ReviewStatus) {
    const status_ = query.status.includes(status)
      ? query.status.filter((s) => s !== status)
      : [...query.status, status];
    onquery({ ...query, status: status_ });
  }

  const filtering = $derived(
    !!(query.q || query.status.length || query.author || query.reviewer || query.task || query.branch),
  );

  function open(e: MouseEvent, reference: string) {
    if (e.button !== 0 || e.shiftKey || e.altKey) return;
    e.preventDefault();
    onopen(reference, e.metaKey || e.ctrlKey);
  }

  function ago(at: string): string {
    const seconds = (Date.now() - Date.parse(at)) / 1000;
    if (!Number.isFinite(seconds)) return at;
    if (seconds < 60) return 'just now';
    const [unit, size] =
      seconds < 3600 ? ['min', 60] : seconds < 86400 ? ['h', 3600] : seconds < 86400 * 30 ? ['d', 86400] : [null, 0];
    return unit ? `${Math.floor(seconds / (size as number))} ${unit} ago` : at.slice(0, 10);
  }
</script>

<div class="review-list" id="review-list">
  <div class="review-list-bar">
    <input
      class="review-search"
      type="search"
      placeholder="Search reviews"
      aria-label="Search reviews"
      value={search}
      oninput={(e) => typed(e.currentTarget.value)}
    />
    {#if waiting.length}
      <button class="review-next" onclick={() => onopen(waiting[0].review.reference, false)}>
        Next review needing you <span class="review-count">{waiting.length}</span>
      </button>
    {/if}
  </div>
  <div class="review-filters" role="group" aria-label="Filters">
    {#each STATUSES as status (status)}
      <button
        class="review-chip status-{status}"
        aria-pressed={query.status.includes(status)}
        data-status={status}
        onclick={() => toggleStatus(status)}>{STATUS_LABELS[status]}</button
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
  {:else if !entries.length}
    <p class="review-empty">
      {loading
        ? 'Loading…'
        : filtering
          ? 'No review fits.'
          : 'No reviews yet. Agents open them with atm review create.'}
    </p>
  {:else}
    <ul class="review-rows" aria-busy={loading}>
      {#each entries as entry (entry.review.id)}
        {@const r = entry.review}
        <li class="review-row" class:needs-you={entry.needs_you} data-review={r.reference}>
          <a class="review-link" href={href(r.reference)} onclick={(e) => open(e, r.reference)}>
            <span class="review-ref">{r.reference}</span>
            <span class="review-title">{r.title}</span>
          </a>
          <span class="review-status status-{r.status}">{STATUS_LABELS[r.status]}</span>
          {#if entry.needs_you}<span class="review-needs" title="Needs you">Needs you</span>{/if}
          <span class="review-meta">
            {#if r.tasks.length}<span class="review-tasks">{r.tasks.join(', ')}</span>{/if}
            {#if entry.branch}<span class="review-branch">{entry.branch}</span>{/if}
            <span>{r.author ?? 'no author yet'}</span>
            <span title="Open chains of comments">{entry.open} open of {entry.chains}</span>
            <span title={r.updated}>{ago(r.updated)}</span>
          </span>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .review-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    padding: var(--space-6);
  }
  .review-list-bar,
  .review-filters {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
  }
  .review-search {
    flex: 1;
    min-width: 12rem;
    padding: var(--space-3) var(--space-4);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg);
    color: var(--text);
    font: inherit;
  }
  .review-next,
  .review-clear,
  .review-chip,
  .review-select {
    padding: var(--space-2) var(--space-4);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: var(--text-sm);
    cursor: pointer;
  }
  .review-next {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  .review-count {
    margin-left: var(--space-2);
    font-weight: 600;
  }
  .review-chip[aria-pressed='true'] {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  .review-rows {
    list-style: none;
    margin: 0;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
  }
  .review-row {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--space-4);
    padding: var(--space-4) var(--space-6);
    border-top: 1px solid var(--border);
  }
  .review-row:first-child {
    border-top: none;
  }
  .review-row.needs-you {
    box-shadow: inset 3px 0 0 var(--accent);
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
    font-weight: 600;
  }
  .review-status,
  .review-needs {
    padding: 0 var(--space-3);
    border-radius: var(--radius-sm);
    font-size: var(--text-xs);
    background: var(--surface);
    border: 1px solid var(--border);
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
  .review-meta {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-4);
    width: 100%;
    color: var(--text-muted);
    font-size: var(--text-xs);
  }
  .review-branch {
    font-family: 'SF Mono', Menlo, Consolas, monospace;
  }
  .review-empty,
  .review-problem {
    color: var(--text-muted);
  }
</style>
