// A review's page's own logic (docs/code-review-design.md §3.11, ATM-574): the files as a tree, what each file's patch
// adds and removes, where its chains go in the round shown, and the key a file's *viewed* tick is kept under.
import type { Chain, PlacedChain, ViewFile } from './types';

/** A row of the file tree: a folder (its path, how deep) or a file of the view */
export type TreeRow =
  | { kind: 'folder'; path: string; name: string; depth: number }
  | { kind: 'file'; file: ViewFile; name: string; depth: number };

/** The files as a tree: folders before their files, each level sorted, a folder shown once */
export function fileTree(files: ViewFile[]): TreeRow[] {
  const rows: TreeRow[] = [];
  const shown = new Set<string>();
  const sorted = [...files].sort((a, b) => {
    const [da, db] = [a.path.split('/'), b.path.split('/')];
    for (let i = 0; i < Math.min(da.length, db.length); i++) {
      const lastA = i === da.length - 1;
      const lastB = i === db.length - 1;
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
  }
  return rows;
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

/** Whether a chain is resolved: each of its top-level comments is (a chain with none isn't) */
export function isResolved(chain: Chain): boolean {
  return chain.comments.length > 0 && chain.comments.every((c) => c.resolved === true);
}

/** Which chains the Comments tab lists */
export type ChainFilter = 'all' | 'open' | 'resolved';

/** The chains a filter keeps, in their order */
export function filterChains<C extends Chain>(chains: C[], filter: ChainFilter): C[] {
  return filter === 'all' ? chains : chains.filter((c) => isResolved(c) === (filter === 'resolved'));
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
