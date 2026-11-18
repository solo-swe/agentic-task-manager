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

/** How often a live review's page is asked for again while it is shown, so it follows the working copy (ATM-576,
 * §3.5: a change shows within a second or two) */
export const LIVE_MS = 2000;

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

/** The sides made, by client and `project@revision`: the same source each time, so a page asked for again (its
 * refresh) shows what it has rather than loading it again */
const sources = new WeakMap<Fetch, Map<string, DocumentSource>>();
/** At most this many sides are kept per client; the oldest go first */
export const SOURCES_CAP = 64;

/** How many sides are asked for at once (ATM-587, measured on CompMD's CR-1: scrolled to its end, the 160 sides of the
 * files passed were asked for at once, so the last file's waited behind all of them, in the browser's queue, almost
 * 2 s). The rest wait, the one asked for last first: the one nearest where the reader is now. */
export const LOADS_AT_ONCE = 4;
const queued: (() => void)[] = [];
let loading = 0;

/** `load` when fewer than LOADS_AT_ONCE are being asked for; else after them, the latest asked first */
function inTurn<T>(load: () => Promise<T>): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const go = () => {
      loading += 1;
      load()
        .then(resolve, reject)
        .finally(() => {
          loading -= 1;
          queued.pop()?.();
        });
    };
    if (loading < LOADS_AT_ONCE) go();
    else queued.push(go);
  });
}

/** For tests: how many sides are being asked for, and how many wait */
export function sideLoads(): { loading: number; waiting: number } {
  return { loading, waiting: queued.length };
}

/** A round's side: the project's files as commit or tree `revision` has them; never changing, so each file is loaded
 * once (a failed load is asked again next time), LOADS_AT_ONCE at a time */
export function revisionDocuments(project: string, revision: string, fetchFn: Fetch = browserFetch): DocumentSource {
  const id = `${project}@${revision}`;
  let made = sources.get(fetchFn);
  if (!made) sources.set(fetchFn, (made = new Map()));
  const known = made.get(id);
  if (known) return known;
  const files = new Map<string, ReturnType<typeof loadDocument>>();
  const source: DocumentSource = {
    id,
    load: (path) => {
      let loading = files.get(path);
      if (!loading) {
        loading = inTurn(() => loadDocument(project, path, fetchFn, undefined, revision));
        files.set(path, loading);
        loading.then(
          (result) => 'error' in result && files.delete(path),
          () => files.delete(path),
        );
      }
      return loading;
    },
    // A rendered side's images show their alt text: the files route reads the working copy, not a revision
    imageUrl: () => null,
    repositoryLink: () => null,
  };
  made.set(id, source);
  if (made.size > SOURCES_CAP) made.delete(made.keys().next().value!);
  return source;
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
