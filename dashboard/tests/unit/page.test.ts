import { describe, expect, it } from 'vitest';
import { chainsOf, driftedOf, fileTree, hashOf, lineCounts, reviewChains, viewedKey } from '../../src/lib/reviews/page';
import type { PlacedChain, ViewFile } from '../../src/lib/reviews/types';

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
