// A review's page's own logic (docs/code-review-design.md §3.11, ATM-574): the files as a tree, what each file's patch
// adds and removes, where its chains go in the round shown, and the key a file's *viewed* tick is kept under.
import type { Chain, PlacedChain, ReviewEntry, ViewFile } from './types';

/** How many days a decided review stays among the recent ones, before *Older* (ATM-610) */
export const RECENT_DAYS = 5;

/** A group of the unfiltered reviews list (ATM-610), in the order the list shows them */
export interface ReviewGroup {
  id: 'waiting' | 'commented' | 'sent-back' | 'others' | 'recent' | 'older';
  title: string;
  entries: ReviewEntry[];
}

const DAY_MS = 86_400_000;
const time = (at: string | null | undefined) => (at ? Date.parse(at) : NaN);
/** Latest first, by the review's last activity */
const latestFirst = (a: ReviewEntry, b: ReviewEntry) => b.review.updated.localeCompare(a.review.updated);

/**
 * The unfiltered list in groups (ATM-610), each review in the first it fits, empty groups left out:
 * 1. *Waiting for your review*: waiting or live, needing you; the longest waiting first (since its latest round);
 * 2. *With open comments*: the rest with open comments, the latest activity first;
 * 3. *Sent back*: changes requested, waiting on their author, the latest first;
 * 4. *Waiting on others*: the rest not decided yet (a draft, or waiting on someone else), the latest first, so no
 *    review is left out;
 * 5. *Recently decided*: approved or closed in the last [`RECENT_DAYS`], the latest first;
 * 6. *Older*: decided before that, the latest first (folded by the list).
 */
export function groupReviews(entries: ReviewEntry[], now: number): ReviewGroup[] {
  const groups: ReviewGroup[] = [
    { id: 'waiting', title: 'Waiting for your review', entries: [] },
    { id: 'commented', title: 'With open comments', entries: [] },
    { id: 'sent-back', title: 'Sent back', entries: [] },
    { id: 'others', title: 'Waiting on others', entries: [] },
    { id: 'recent', title: 'Recently decided', entries: [] },
    { id: 'older', title: 'Older', entries: [] },
  ];
  const into = (id: ReviewGroup['id'], entry: ReviewEntry) => groups.find((g) => g.id === id)!.entries.push(entry);
  for (const entry of entries) {
    const status = entry.review.status;
    const decided = status === 'approved' || status === 'closed';
    if (entry.needs_you && (status === 'waiting' || status === 'live')) into('waiting', entry);
    else if (entry.open > 0) into('commented', entry);
    else if (status === 'changes_requested') into('sent-back', entry);
    else if (!decided) into('others', entry);
    else if (now - time(entry.review.updated) <= RECENT_DAYS * DAY_MS) into('recent', entry);
    else into('older', entry);
  }
  for (const group of groups) {
    if (group.id === 'waiting') {
      // The longest waiting first; one with no time (an older atm's list) after them
      const since = (e: ReviewEntry) => time(e.waiting_since ?? e.review.updated);
      group.entries.sort((a, b) => (since(a) || Infinity) - (since(b) || Infinity));
    } else group.entries.sort(latestFirst);
  }
  return groups.filter((g) => g.entries.length);
}

/** Several projects' reviews as one list (ATM-575, the overview's Reviews): each tagged with its project, the latest
 * activity first, as one project's list is */
export function mergeProjectReviews(
  lists: { project: { id: string; name: string }; entries: ReviewEntry[] }[],
): ReviewEntry[] {
  return lists.flatMap(({ project, entries }) => entries.map((e) => ({ ...e, project }))).sort(latestFirst);
}

/** How long a review has waited, in words: "under an hour", "5 hours", "1 day", "3 days" (ATM-610) */
export function waitedFor(since: string, now: number): string {
  const ms = now - Date.parse(since);
  if (!Number.isFinite(ms) || ms < 3_600_000) return 'under an hour';
  if (ms < DAY_MS) {
    const hours = Math.floor(ms / 3_600_000);
    return `${hours} ${hours === 1 ? 'hour' : 'hours'}`;
  }
  const days = Math.floor(ms / DAY_MS);
  return `${days} ${days === 1 ? 'day' : 'days'}`;
}

/** A row of the file tree: a folder (its path, how deep) or a file of the view */
export type TreeRow =
  | { kind: 'folder'; path: string; name: string; depth: number }
  | { kind: 'file'; file: ViewFile; name: string; depth: number };

/** The files as a tree: folders before their files, each level sorted, a folder shown once */
export function fileTree(files: ViewFile[]): TreeRow[] {
  const rows: TreeRow[] = [];
  const shown = new Set<string>();
  // A submodule's entry stands where its folder is, the files changed inside it under it (ATM-599): sorted as a
  // folder, and its row is the folder's
  const sorted = [...files].sort((a, b) => {
    const [da, db] = [a.path.split('/'), b.path.split('/')];
    for (let i = 0; i < Math.min(da.length, db.length); i++) {
      const lastA = i === da.length - 1 && !a.submodule;
      const lastB = i === db.length - 1 && !b.submodule;
      // A folder before a file at the same level
      if (lastA !== lastB) return lastA ? 1 : -1;
      if (da[i] !== db[i]) return da[i] < db[i] ? -1 : 1;
    }
    return da.length - db.length;
  });
  for (const file of sorted) {
    const parts = file.path.split('/');
    for (let depth = 0; depth < parts.length - 1; depth++) {
      const path = parts.slice(0, depth + 1).join('/');
      if (shown.has(path)) continue;
      shown.add(path);
      rows.push({ kind: 'folder', path, name: parts[depth], depth });
    }
    rows.push({ kind: 'file', file, name: parts.at(-1) ?? file.path, depth: parts.length - 1 });
    if (file.submodule) shown.add(file.path);
  }
  return rows;
}

/** The files in the tree's order, top to bottom (ATM-596): the order of the diffs, of moving between files and of
 * everything else the page lists files in, so the right side reads as the tree does */
export function treeOrder(files: ViewFile[]): ViewFile[] {
  return fileTree(files).flatMap((row) => (row.kind === 'file' ? [row.file] : []));
}

/** A row's path: a folder's own, a file's */
const rowPath = (row: TreeRow) => (row.kind === 'folder' ? row.path : row.file.path);

/** Whether `path` is inside the folder `folder` */
const inside = (path: string, folder: string) => path.startsWith(folder + '/');

/** The rows shown with some folders collapsed (ATM-595): a collapsed folder's row stays, what is inside it hides */
export function shownRows(rows: TreeRow[], collapsed: ReadonlySet<string>): TreeRow[] {
  if (!collapsed.size) return rows;
  return rows.filter((row) => {
    const path = rowPath(row);
    for (const folder of collapsed) if (inside(path, folder)) return false;
    return true;
  });
}

/** Every folder of the tree, by path */
export function treeFolders(rows: TreeRow[]): string[] {
  return rows.flatMap((row) => (row.kind === 'folder' ? [row.path] : []));
}

/** The folders collapsed, with those on the way to `path` opened, so its row is shown; the same set if none was */
export function openedTo(collapsed: ReadonlySet<string>, path: string): ReadonlySet<string> {
  const on = [...collapsed].filter((folder) => inside(path, folder));
  if (!on.length) return collapsed;
  return new Set([...collapsed].filter((folder) => !on.includes(folder)));
}

/** The files under a folder, at any depth */
export function filesIn(rows: TreeRow[], folder: string): ViewFile[] {
  return rows.flatMap((row) => (row.kind === 'file' && inside(row.file.path, folder) ? [row.file] : []));
}

/** How many lines a file's patch adds and removes (its hunks' lines, not its headers) */
export function lineCounts(patch: string): { added: number; removed: number } {
  let added = 0;
  let removed = 0;
  let inHunk = false;
  for (const line of patch.split('\n')) {
    if (line.startsWith('@@')) inHunk = true;
    else if (line.startsWith('diff --git ')) inHunk = false;
    else if (inHunk && line.startsWith('+')) added++;
    else if (inHunk && line.startsWith('-')) removed++;
  }
  return { added, removed };
}

/** A file's chains as the diff viewer places them: at their lines in the round shown (a drifted one at its last
 * place), a whole file's anywhere */
export function chainsOf(chains: PlacedChain[], file: ViewFile): Chain[] {
  return chains
    .filter((c) => c.anchor.path === file.path || (file.old_path !== file.path && c.anchor.path === file.old_path))
    .map((c) => ({ id: c.id, comments: c.comments, anchor: { ...c.anchor, path: file.path, lines: c.placed } }));
}

/** The chains of the review as a whole (on no file) */
export function reviewChains(chains: PlacedChain[]): PlacedChain[] {
  return chains.filter((c) => !c.anchor.path);
}

/** A file's drifted chains: their text is gone from the round shown, and they link to where they were made */
export function driftedOf(chains: PlacedChain[], file: ViewFile): PlacedChain[] {
  return chains.filter((c) => c.drifted && (c.anchor.path === file.path || c.anchor.path === file.old_path));
}

/** Whether a chain is open, as atm's rules count it: one of its top-level comments has its box unticked. A poke has no
 * box (`resolved` null, ATM-611), so a chain of pokes is never open, and never keeps the review from an approval */
export function isOpen(chain: Chain): boolean {
  return chain.comments.some((c) => c.resolved === false);
}

/** Whether a chain is resolved: a box ticked, and none left open (a chain of pokes is neither) */
export function isResolved(chain: Chain): boolean {
  return chain.comments.some((c) => c.resolved === true) && !isOpen(chain);
}

/** Which chains the Comments tab lists */
export type ChainFilter = 'all' | 'open' | 'resolved';

/** The chains a filter keeps, in their order */
export function filterChains<C extends Chain>(chains: C[], filter: ChainFilter): C[] {
  return filter === 'all' ? chains : chains.filter(filter === 'open' ? isOpen : isResolved);
}

/** A review's page's title, the window's and the tab's: the project's short name, the review's reference, its title */
export function reviewTitle(project: string | null | undefined, reference: string, title: string): string {
  return [project, reference].filter(Boolean).join(' ') + (title ? `: ${title}` : '');
}

/** The file tree's width, kept between its least and at most half the page */
export const TREE_WIDTH = { least: 160, default: 280 } as const;
export function treeWidth(width: number, page: number): number {
  const most = Math.max(TREE_WIDTH.least, Math.floor(page / 2));
  return Math.round(Math.min(most, Math.max(TREE_WIDTH.least, Number.isFinite(width) ? width : TREE_WIDTH.default)));
}

/** A short hash of a text (FNV-1a), for a key that changes with a file's change */
export function hashOf(text: string): string {
  let hash = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    hash ^= text.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  return (hash >>> 0).toString(16);
}

/** The key a file's *viewed* tick is kept under: its review, the view (round, against), the file and its change, so a
 * tick doesn't carry over to a file that changed */
export function viewedKey(review: string, round: number, against: number | null, file: ViewFile): string {
  return [review, round, against ?? '', file.path, hashOf(file.patch)].join(':');
}
