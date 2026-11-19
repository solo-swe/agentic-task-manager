import { describe, expect, it } from 'vitest';
import {
  RECENT_DAYS,
  TREE_WIDTH,
  chainsOf,
  driftedOf,
  fileTree,
  filesIn,
  filterChains,
  groupReviews,
  hashOf,
  isOpen,
  isResolved,
  lineCounts,
  mergeProjectReviews,
  openedTo,
  reviewChains,
  reviewTitle,
  shownRows,
  treeFolders,
  treeOrder,
  treeWidth,
  viewedKey,
  waitedFor,
  type ChainFilter,
} from '../../src/lib/reviews/page';
import type { PlacedChain, ReviewEntry, ReviewStatus, ViewFile } from '../../src/lib/reviews/types';

// ATM-610: the unfiltered reviews list in groups, what waits for you first
describe('the reviews list in groups', () => {
  const NOW = Date.parse('2026-11-19T12:00:00Z');
  const daysAgo = (d: number) => new Date(NOW - d * 86_400_000).toISOString();
  const entry = (
    ref: string,
    status: ReviewStatus,
    more: { needs_you?: boolean; open?: number; updated?: string; since?: string | null } = {},
  ): ReviewEntry => ({
    review: {
      id: ref,
      reference: ref,
      number: 1,
      title: ref,
      status,
      tasks: [],
      author: 'claude:a',
      reviewers: ['Hakan'],
      created: daysAgo(30),
      updated: more.updated ?? daysAgo(1),
    },
    branch: null,
    chains: more.open ?? 0,
    open: more.open ?? 0,
    needs_you: more.needs_you ?? false,
    waiting_since: more.since === undefined ? daysAgo(1) : more.since,
  });
  const shape = (entries: ReviewEntry[]) =>
    groupReviews(entries, NOW).map((g) => [g.id, g.entries.map((e) => e.review.reference)]);

  it('each review in the first group it fits, empty groups left out', () => {
    expect(
      shape([
        entry('CR-1', 'waiting', { needs_you: true, open: 2 }), // waits on you: first, though it has open comments
        entry('CR-2', 'waiting', { open: 1 }),
        entry('CR-3', 'changes_requested', { open: 1 }), // open comments come before sent back
        entry('CR-4', 'changes_requested'),
        entry('CR-5', 'draft'),
        entry('CR-6', 'approved', { updated: daysAgo(2) }),
        entry('CR-7', 'closed', { updated: daysAgo(20) }),
        entry('CR-8', 'live', { needs_you: true }),
        entry('CR-9', 'changes_requested', { needs_you: true }), // its author's to answer: not waiting for review
      ]),
    ).toEqual([
      ['waiting', ['CR-1', 'CR-8']],
      ['commented', ['CR-2', 'CR-3']],
      ['sent-back', ['CR-4', 'CR-9']],
      ['others', ['CR-5']],
      ['recent', ['CR-6']],
      ['older', ['CR-7']],
    ]);
    expect(shape([entry('CR-1', 'draft')])).toEqual([['others', ['CR-1']]]);
    expect(shape([])).toEqual([]);
  });

  it('waiting for you: the longest waiting on top, by its latest round; one without a time after them', () => {
    expect(
      shape([
        entry('CR-1', 'waiting', { needs_you: true, since: daysAgo(1) }),
        entry('CR-2', 'waiting', { needs_you: true, since: null, updated: daysAgo(9) }),
        entry('CR-3', 'waiting', { needs_you: true, since: daysAgo(4) }),
      ]),
    ).toEqual([['waiting', ['CR-2', 'CR-3', 'CR-1']]]);
    const noTime = entry('CR-4', 'waiting', { needs_you: true, since: null, updated: 'never' });
    expect(shape([noTime, entry('CR-1', 'waiting', { needs_you: true })])).toEqual([['waiting', ['CR-1', 'CR-4']]]);
  });

  it('the other groups: the latest activity first', () => {
    expect(
      shape([
        entry('CR-1', 'approved', { updated: daysAgo(3) }),
        entry('CR-2', 'approved', { updated: daysAgo(1) }),
        entry('CR-3', 'closed', { updated: daysAgo(2) }),
      ]),
    ).toEqual([['recent', ['CR-2', 'CR-3', 'CR-1']]]);
  });

  it(`decided within ${RECENT_DAYS} days is recent; past that, older`, () => {
    expect(
      shape([
        entry('CR-1', 'approved', { updated: daysAgo(RECENT_DAYS) }),
        entry('CR-2', 'approved', { updated: daysAgo(RECENT_DAYS + 0.01) }),
      ]),
    ).toEqual([
      ['recent', ['CR-1']],
      ['older', ['CR-2']],
    ]);
  });

  // ATM-575: the overview's list of every project's reviews
  it('several projects’ reviews as one list: each with its project, the latest activity first', () => {
    const a = { id: 'p1', name: 'atm' };
    const b = { id: 'p2', name: 'CompMD' };
    const merged = mergeProjectReviews([
      { project: a, entries: [entry('CR-1', 'waiting', { updated: daysAgo(3) })] },
      { project: b, entries: [entry('CR-1', 'waiting', { updated: daysAgo(1) }), entry('CR-2', 'draft')] },
      { project: { id: 'p3', name: 'Empty' }, entries: [] },
    ]);
    expect(merged.map((e) => [e.project?.name, e.review.reference])).toEqual([
      ['CompMD', 'CR-1'],
      ['CompMD', 'CR-2'],
      ['atm', 'CR-1'],
    ]);
    expect(mergeProjectReviews([])).toEqual([]);
  });

  it('how long a review has waited, in words', () => {
    const at = (ms: number) => new Date(NOW - ms).toISOString();
    expect(waitedFor(at(10 * 60_000), NOW)).toBe('under an hour');
    expect(waitedFor(at(3_600_000), NOW)).toBe('1 hour');
    expect(waitedFor(at(5 * 3_600_000), NOW)).toBe('5 hours');
    expect(waitedFor(at(86_400_000), NOW)).toBe('1 day');
    expect(waitedFor(at(3.5 * 86_400_000), NOW)).toBe('3 days');
    expect(waitedFor('not a time', NOW)).toBe('under an hour');
  });
});

// A review's page's own logic (lib/reviews/page.ts, ATM-574)

const file = (path: string, patch = '', more: Partial<ViewFile> = {}): ViewFile => ({
  path,
  old_path: path,
  status: 'modified',
  binary: false,
  patch,
  before: 'b',
  after: 'a',
  ...more,
});

const chain = (id: string, path: string | null, placed: [number, number] | null, more: Partial<PlacedChain> = {}) =>
  ({
    id,
    anchor: { round: 1, path, side: 'new', lines: [1, 2], quote: 'q' },
    comments: [],
    placed,
    drifted: null,
    ...more,
  }) as PlacedChain;

describe('the file tree', () => {
  it('is empty with no files, and a file at the root has no folder', () => {
    expect(fileTree([])).toEqual([]);
    expect(fileTree([file('README.md')])).toEqual([
      { kind: 'file', file: file('README.md'), name: 'README.md', depth: 0 },
    ]);
  });

  it('shows each folder once, before its files, folders before files at each level, each level sorted', () => {
    const rows = fileTree([
      file('src/b.rs'),
      file('README.md'),
      file('src/a/x.rs'),
      file('src/a.rs'),
      file('docs/z.md'),
    ]).map((r) => (r.kind === 'folder' ? `${'  '.repeat(r.depth)}${r.name}/` : `${'  '.repeat(r.depth)}${r.name}`));
    expect(rows).toEqual(['docs/', '  z.md', 'src/', '  a/', '    x.rs', '  a.rs', '  b.rs', 'README.md']);
  });

  it('keeps every file, deep paths included, each with its own depth', () => {
    const tree = fileTree([file('a/b/c/d.txt'), file('a/e.txt')]);
    expect(tree.filter((r) => r.kind === 'file').map((r) => [r.name, r.depth])).toEqual([
      ['d.txt', 3],
      ['e.txt', 1],
    ]);
    expect(tree.filter((r) => r.kind === 'folder').map((r) => (r.kind === 'folder' ? r.path : ''))).toEqual([
      'a',
      'a/b',
      'a/b/c',
    ]);
  });

  // ATM-599: a submodule moved to another commit, and the files changed inside it
  it('a submodule’s entry stands where its folder is, among the folders, its files under it', () => {
    const sub = { from: 'a1', to: 'b2', repository: null };
    const rows = fileTree([
      file('src/x.rs'),
      file('lib/b.txt'),
      file('lib', '', { submodule: sub }),
      file('lib/a.txt'),
      file('README.md'),
    ]).map((r) => `${'  '.repeat(r.depth)}${r.kind === 'folder' ? `${r.name}/` : r.name}`);
    expect(rows).toEqual(['lib', '  a.txt', '  b.txt', 'src/', '  x.rs', 'README.md']);
  });
});

// ATM-596: the diffs follow the tree, not the patch's order by path
describe('the files in the tree’s order', () => {
  const paths = (files: ViewFile[]) => files.map((f) => f.path);
  it('none, and one', () => {
    expect(treeOrder([])).toEqual([]);
    expect(paths(treeOrder([file('README.md')]))).toEqual(['README.md']);
  });
  it('folders before the files beside them, each level by name; a lock file at the root last', () => {
    // As git lists them: by path, so the root's files sit between its folders
    const git = [
      'Cargo.toml',
      'crates/a/src/lib.rs',
      'crates/a/Cargo.toml',
      'docs/plan.md',
      'pnpm-lock.yaml',
      'src/main.rs',
    ].map((p) => file(p));
    const ordered = treeOrder(git);
    expect(paths(ordered)).toEqual([
      'crates/a/src/lib.rs',
      'crates/a/Cargo.toml',
      'docs/plan.md',
      'src/main.rs',
      'Cargo.toml',
      'pnpm-lock.yaml',
    ]);
    // The tree reads the same, top to bottom
    const inTree = fileTree(git).flatMap((row) => (row.kind === 'file' ? [row.file.path] : []));
    expect(paths(ordered)).toEqual(inTree);
  });
  it('a submodule stands where its folder is', () => {
    const ordered = treeOrder([
      file('z.txt'),
      file('vendor/lib', '', { submodule: { from: 'a1', to: 'b2', repository: null } }),
      file('a/b.txt'),
    ]);
    expect(paths(ordered)).toEqual(['a/b.txt', 'vendor/lib', 'z.txt']);
  });
});

// ATM-595: the tree's folders fold
describe('the file tree folded', () => {
  const tree = fileTree([file('a/b/c.txt'), file('a/d.txt'), file('ab/e.txt'), file('f.txt')]);
  const names = (rows: ReturnType<typeof fileTree>) =>
    rows.map((r) => (r.kind === 'folder' ? `${r.path}/` : r.file.path));

  it('shows everything with nothing folded, and a folded folder’s row without what is inside it, at any depth', () => {
    expect(shownRows(tree, new Set())).toBe(tree);
    expect(names(shownRows(tree, new Set(['a'])))).toEqual(['a/', 'ab/', 'ab/e.txt', 'f.txt']);
    expect(names(shownRows(tree, new Set(['a/b'])))).toEqual(['a/', 'a/b/', 'a/d.txt', 'ab/', 'ab/e.txt', 'f.txt']);
    // A folder named like the start of another isn't its parent
    expect(names(shownRows(tree, new Set(['a', 'a/b'])))).toEqual(['a/', 'ab/', 'ab/e.txt', 'f.txt']);
  });

  it('lists every folder, and the files under one at any depth', () => {
    expect(treeFolders(tree)).toEqual(['a', 'a/b', 'ab']);
    expect(filesIn(tree, 'a').map((f) => f.path)).toEqual(['a/b/c.txt', 'a/d.txt']);
    expect(filesIn(tree, 'a/b').map((f) => f.path)).toEqual(['a/b/c.txt']);
    expect(filesIn(tree, 'nowhere')).toEqual([]);
  });

  it('opens the folders on the way to a file, and leaves the same set when none was folded on its way', () => {
    const folded = new Set(['a', 'a/b', 'ab']);
    expect([...openedTo(folded, 'a/b/c.txt')]).toEqual(['ab']);
    expect([...openedTo(folded, 'a/d.txt')]).toEqual(['a/b', 'ab']);
    expect(openedTo(folded, 'f.txt')).toBe(folded);
  });
});

describe('a file’s line counts', () => {
  it('counts its hunks’ added and removed lines, not its headers', () => {
    const patch = [
      'diff --git a/x b/x',
      '--- a/x',
      '+++ b/x',
      '@@ -1,3 +1,3 @@',
      ' kept',
      '-gone',
      '+new',
      '+more',
      '@@ -9 +10 @@',
      '-also gone',
      '',
    ].join('\n');
    expect(lineCounts(patch)).toEqual({ added: 2, removed: 2 });
  });

  it('is nothing for no patch, a binary file’s, or a mode change', () => {
    expect(lineCounts('')).toEqual({ added: 0, removed: 0 });
    expect(lineCounts('diff --git a/i.png b/i.png\nBinary files a/i.png and b/i.png differ\n')).toEqual({
      added: 0,
      removed: 0,
    });
    expect(lineCounts('diff --git a/s b/s\nold mode 100644\nnew mode 100755\n')).toEqual({ added: 0, removed: 0 });
  });

  it('counts a line of +++ or --- in a hunk as content', () => {
    expect(lineCounts('--- a/x\n+++ b/x\n@@ -1 +1 @@\n----\n++++\n')).toEqual({ added: 1, removed: 1 });
  });
});

describe('a file’s chains', () => {
  const chains = [
    chain('t1', 'a.md', [3, 4]),
    chain('t2', 'b.md', [1, 1]),
    chain('t3', null, null),
    chain('t4', 'a.md', [9, 9], { drifted: { round: 1, lines: [7, 7] } }),
    chain('t5', 'a.md', null),
    chain('t6', 'old.md', [2, 2]),
  ];

  it('are the file’s, at their lines in the round shown (a drifted one at its last place, a whole file’s nowhere)', () => {
    const of = chainsOf(chains, file('a.md'));
    expect(of.map((c) => [c.id, c.anchor.lines])).toEqual([
      ['t1', [3, 4]],
      ['t4', [9, 9]],
      ['t5', null],
    ]);
    expect(of.every((c) => c.anchor.path === 'a.md')).toBe(true);
  });

  it('follow a file that moved: its chains under its old path are its own', () => {
    const moved = file('new.md', '', { old_path: 'old.md', status: 'renamed' });
    expect(chainsOf(chains, moved).map((c) => [c.id, c.anchor.path])).toEqual([['t6', 'new.md']]);
  });

  it('none for a file no chain is on', () => {
    expect(chainsOf(chains, file('c.md'))).toEqual([]);
    expect(driftedOf(chains, file('c.md'))).toEqual([]);
  });

  it('drifted: only those whose text is gone, of this file', () => {
    expect(driftedOf(chains, file('a.md')).map((c) => c.id)).toEqual(['t4']);
    expect(driftedOf(chains, file('b.md'))).toEqual([]);
  });

  it('the review’s own are on no file', () => {
    expect(reviewChains(chains).map((c) => c.id)).toEqual(['t3']);
    expect(reviewChains([])).toEqual([]);
  });
});

describe('a viewed tick’s key', () => {
  it('names the review, the view and the file, and changes with the file’s change', () => {
    const f = file('a.md', '@@ -1 +1 @@\n-a\n+b\n');
    const key = viewedKey('r1', 2, null, f);
    expect(key).toBe(`r1:2::a.md:${hashOf(f.patch)}`);
    expect(viewedKey('r1', 2, 1, f)).not.toBe(key);
    expect(viewedKey('r1', 3, null, f)).not.toBe(key);
    expect(viewedKey('r2', 2, null, f)).not.toBe(key);
    expect(viewedKey('r1', 2, null, { ...f, path: 'b.md' })).not.toBe(key);
    expect(viewedKey('r1', 2, null, { ...f, patch: f.patch + '+c\n' })).not.toBe(key);
    expect(viewedKey('r1', 2, null, { ...f })).toBe(key);
  });

  it('hashes any text to a short hex, the same for the same text', () => {
    for (const text of ['', 'a', 'ünïcode ✓', 'x'.repeat(10_000)]) {
      expect(hashOf(text)).toMatch(/^[0-9a-f]{1,8}$/);
      expect(hashOf(text)).toBe(hashOf(text));
    }
    expect(hashOf('a')).not.toBe(hashOf('b'));
  });
});

describe('the Comments tab’s filter', () => {
  const comment = (resolved: boolean | null) => ({ id: 'c', author: 'a', at: '', body: '', resolved, replies: [] });
  const withComments = (id: string, ...resolved: (boolean | null)[]) =>
    chain(id, null, null, { comments: resolved.map(comment) });

  // As atm's rules count them: open is a box unticked; a poke has none (null), so its chain is neither (ATM-643)
  it('a chain is open with a box unticked, resolved with a box ticked and none open; a poke’s is neither', () => {
    for (const [id, boxes, open, resolved] of [
      ['a', [true], false, true],
      ['b', [true, true], false, true],
      ['c', [true, false], true, false],
      ['d', [false], true, false],
      ['poke', [null], false, false],
      ['poke and open', [null, false], true, false],
      ['poke and resolved', [null, true], false, true],
      ['none', [], false, false],
    ] as const) {
      const c = withComments(id, ...boxes);
      expect([isOpen(c), isResolved(c)], id).toEqual([open, resolved]);
    }
  });

  it('keeps all, the open or the resolved ones, in their order; a poke’s chain in all only; nothing from nothing', () => {
    const chains = [
      withComments('a', true),
      withComments('b', false),
      withComments('poke', null),
      withComments('c', true),
      withComments('d', false),
    ];
    const ids = (filter: ChainFilter) => filterChains(chains, filter).map((c) => c.id);
    expect(ids('all')).toEqual(['a', 'b', 'poke', 'c', 'd']);
    expect(ids('open')).toEqual(['b', 'd']);
    expect(ids('resolved')).toEqual(['a', 'c']);
    for (const filter of ['all', 'open', 'resolved'] as const) expect(filterChains([], filter)).toEqual([]);
  });
});

describe('a review’s page title', () => {
  it('the project’s short name, the reference and the title; each left out when there is none', () => {
    expect(reviewTitle('ATM', 'CR-7', 'Trim the names')).toBe('ATM CR-7: Trim the names');
    expect(reviewTitle(null, 'CR-7', 'Trim the names')).toBe('CR-7: Trim the names');
    expect(reviewTitle('', 'CR-7', 'Trim the names')).toBe('CR-7: Trim the names');
    expect(reviewTitle('ATM', 'CR-7', '')).toBe('ATM CR-7');
    expect(reviewTitle(undefined, 'CR-8.1', '')).toBe('CR-8.1');
  });
});

describe('the file tree’s width', () => {
  it('kept between its least and half the page, whole pixels; anything not a number is the default', () => {
    expect(treeWidth(300, 1200)).toBe(300);
    expect(treeWidth(300.6, 1200)).toBe(301);
    expect(treeWidth(100, 1200)).toBe(TREE_WIDTH.least);
    expect(treeWidth(TREE_WIDTH.least, 1200)).toBe(TREE_WIDTH.least);
    expect(treeWidth(900, 1200)).toBe(600);
    expect(treeWidth(600, 1200)).toBe(600);
    expect(treeWidth(Number.NaN, 1200)).toBe(TREE_WIDTH.default);
    expect(treeWidth(Number.POSITIVE_INFINITY, 1200)).toBe(TREE_WIDTH.default);
  });

  it('a page too narrow for half still leaves the tree its least', () => {
    expect(treeWidth(250, 200)).toBe(TREE_WIDTH.least);
    expect(treeWidth(250, 0)).toBe(TREE_WIDTH.least);
  });
});
