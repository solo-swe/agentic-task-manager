// The task manager's side of the review tool (ATM-574): the reviews' reads through the client layer (the REST API, or
// the app's commands), a round's sides as the document viewer's sources, and the files' *viewed* ticks, kept in this
// browser by review, round and file content (docs/code-review-design.md §3.11).
import { type Fetch, browserFetch, reviewUrl, reviewsUrl } from './api/http';
import { loadDocument } from './documents';
import type { DocumentSource } from './documents/source';
import { readStored, writeStored } from './prefs';
import { viewedKey } from './reviews/page';
import type { ListQuery, ReviewList, ReviewPageData, ViewFile } from './reviews/types';

async function getJson<T>(url: string, fetchFn: Fetch): Promise<T> {
  const resp = await fetchFn(url, { cache: 'no-store' });
  const body = (await resp.json().catch(() => ({}))) as { error?: unknown };
  if (!resp.ok) throw new Error(typeof body.error === 'string' ? body.error : `HTTP ${resp.status}`);
  return body as T;
}

/** Project `project`'s reviews the query keeps */
export function loadReviews(project: string, query: ListQuery, fetchFn: Fetch = browserFetch): Promise<ReviewList> {
  return getJson(reviewsUrl(project, query), fetchFn);
}

/** A review's page for one view */
export function loadReview(
  project: string,
  review: string,
  round: number | null,
  against: number | null,
  fetchFn: Fetch = browserFetch,
): Promise<ReviewPageData> {
  return getJson(reviewUrl(project, review, round, against), fetchFn);
}

/** A round's side: the project's files as commit or tree `revision` has them; never changing, so loaded once */
export function revisionDocuments(project: string, revision: string, fetchFn: Fetch = browserFetch): DocumentSource {
  return {
    id: `${project}@${revision}`,
    load: (path) => loadDocument(project, path, fetchFn, undefined, revision),
    // A rendered side's images show their alt text: the files route reads the working copy, not a revision
    imageUrl: () => null,
    repositoryLink: () => null,
  };
}

/** Where the ticks are kept in this browser */
export const VIEWED_KEY = 'atm-review-viewed';
/** At most this many ticks are kept; the oldest go first */
export const VIEWED_CAP = 5000;

/** The files ticked as viewed, kept by their keys (`viewedKey`) in the order they were ticked */
export function viewedKeys(): string[] {
  const stored = readStored(VIEWED_KEY).keys;
  return Array.isArray(stored) ? stored.filter((k): k is string => typeof k === 'string') : [];
}

/** Whether `file` of a review's view is ticked */
export function isViewed(
  keys: readonly string[],
  review: string,
  round: number,
  against: number | null,
  file: ViewFile,
) {
  return keys.includes(viewedKey(review, round, against, file));
}

/** `file` ticked or unticked; the keys after */
export function toggleViewed(review: string, round: number, against: number | null, file: ViewFile): string[] {
  const key = viewedKey(review, round, against, file);
  const keys = viewedKeys();
  const next = keys.includes(key) ? keys.filter((k) => k !== key) : [...keys, key].slice(-VIEWED_CAP);
  writeStored(VIEWED_KEY, { keys: next });
  return next;
}
