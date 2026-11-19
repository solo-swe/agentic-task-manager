import { readStored, writeStored } from './prefs';
import { NO_QUERY, STATUS_LABELS, type ListQuery, type StatusFilter } from './reviews/types';

// The reviews list's last filter, kept in this browser per project (ATM-604): the list opens with it. Its statuses,
// author, reviewer, task and branch; not the search words, which are for the moment.

export const REVIEW_FILTER_KEY = 'atm-review-filter';

const TEXTS = ['author', 'reviewer', 'task', 'branch'] as const;

/** The filter last kept for `project`, or none; anything stored that isn't one is left out. */
export function keptFilter(project: string): ListQuery {
  const kept = readStored(REVIEW_FILTER_KEY)[project];
  const query: ListQuery = { ...NO_QUERY, status: [] };
  if (!kept || typeof kept !== 'object' || Array.isArray(kept)) return query;
  const from = kept as Record<string, unknown>;
  // A status, or open (every review not decided yet: what a filter kept before ATM-613 meant by it, near enough)
  if (Array.isArray(from.status))
    query.status = from.status.filter(
      (s): s is StatusFilter => typeof s === 'string' && (s === 'open' || Object.hasOwn(STATUS_LABELS, s)),
    );
  for (const key of TEXTS) if (typeof from[key] === 'string') query[key] = from[key];
  return query;
}

/** Keeps `query`'s filter for `project` (its search words aside); an empty filter forgets the project's. */
export function keepFilter(project: string, query: ListQuery): void {
  const all = readStored(REVIEW_FILTER_KEY);
  const filter = { status: query.status, ...Object.fromEntries(TEXTS.map((k) => [k, query[k]])) };
  const empty = !query.status.length && TEXTS.every((k) => !query[k]);
  if (empty) delete all[project];
  else all[project] = filter;
  writeStored(REVIEW_FILTER_KEY, Object.keys(all).length ? all : null);
}
