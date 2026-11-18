import { describe, expect, it } from 'vitest';
import {
  changeStarts,
  changedLines,
  fenced,
  foldKey,
  linesOf,
  linesUnder,
  nextChange,
  parseHunks,
  splitBlocks,
  unifiedBlocks,
} from '../../src/lib/reviews/diff';

// A ten-line file whose line 3 changed and whose line 8 was removed and two lines added in its place
const NEW =
  ['one', 'two', 'THREE', 'four', 'five', 'six', 'seven', 'eight a', 'eight b', 'nine', 'ten'].join('\n') + '\n';
const PATCH = [
  'diff --git a/f.txt b/f.txt',
  '--- a/f.txt',
  '+++ b/f.txt',
  '@@ -2,3 +2,3 @@',
  ' two',
  '-three',
  '+THREE',
  ' four',
  '@@ -7,3 +7,4 @@ a function',
  ' seven',
  '-eight',
  '+eight a',
  '+eight b',
  ' nine',
  '\\ No newline at end of file',
  '',
].join('\n');

describe('parseHunks', () => {
  it('reads each hunk’s place and lines, and skips the headers and the no-newline mark', () => {
    const hunks = parseHunks(PATCH);
    expect(hunks.map((h) => [h.oldStart, h.oldLines, h.newStart, h.newLines])).toEqual([
      [2, 3, 2, 3],
      [7, 3, 7, 4],
    ]);
    expect(hunks[1].lines.map((l) => l.op + l.text)).toEqual([' seven', '-eight', '+eight a', '+eight b', ' nine']);
    // A count of 1 may be left out: @@ -5 +5 @@
    expect(parseHunks('@@ -5 +5 @@\n-a\n+b\n')[0]).toMatchObject({ oldLines: 1, newLines: 1 });
  });
});

describe('the split view', () => {
  it('pairs removed and added lines, and folds the unchanged runs before, between and after', () => {
    const blocks = splitBlocks(parseHunks(PATCH), NEW);
    expect(blocks.map((b) => (b.type === 'fold' ? `fold ${b.fold.newFrom}+${b.fold.count}` : 'rows'))).toEqual([
      'fold 1+1',
      'rows',
      'fold 5+2',
      'rows',
      'fold 11+1',
    ]);
    const second = blocks[3];
    if (second.type !== 'rows') throw new Error('rows expected');
    expect(second.rows.map((r) => [r.kind, r.old, r.new, r.oldText, r.newText])).toEqual([
      ['context', 7, 7, 'seven', 'seven'],
      ['changed', 8, 8, 'eight', 'eight a'],
      ['added', null, 9, null, 'eight b'],
      ['context', 9, 10, 'nine', 'nine'],
    ]);
  });

  it('show more opens a fold into the file’s lines, joined to the rows around it', () => {
    const hunks = parseHunks(PATCH);
    const folded = splitBlocks(hunks, NEW);
    const middle = folded[2];
    if (middle.type !== 'fold') throw new Error('a fold expected');
    const open = splitBlocks(hunks, NEW, new Set([foldKey(middle.fold)]));
    expect(open.map((b) => b.type)).toEqual(['fold', 'rows', 'fold']);
    const rows = open[1].type === 'rows' ? open[1].rows : [];
    expect(rows.filter((r) => r.new === 5 || r.new === 6).map((r) => [r.old, r.new, r.newText])).toEqual([
      [5, 5, 'five'],
      [6, 6, 'six'],
    ]);
  });
});

describe('the unified view', () => {
  it('lists each hunk’s lines in order, numbered on their side', () => {
    const blocks = unifiedBlocks(parseHunks(PATCH), NEW);
    const first = blocks[1];
    if (first.type !== 'rows') throw new Error('rows expected');
    expect(first.rows.map((r) => [r.kind, r.old, r.new, r.text])).toEqual([
      ['context', 2, 2, 'two'],
      ['removed', 3, null, 'three'],
      ['added', null, 3, 'THREE'],
      ['context', 4, 4, 'four'],
    ]);
  });

  it('a new file is all added; a file with no hunks is one fold', () => {
    const added = unifiedBlocks(parseHunks('@@ -0,0 +1,2 @@\n+a\n+b\n'), 'a\nb\n');
    expect(added).toHaveLength(1);
    expect(unifiedBlocks([], 'a\nb\n')).toEqual([{ type: 'fold', fold: { oldFrom: 1, newFrom: 1, count: 2 } }]);
  });
});

describe('changes, keys and marks', () => {
  it('n and p go from change to change, wrapping round', () => {
    const blocks = splitBlocks(parseHunks(PATCH), NEW);
    const starts = changeStarts(blocks);
    expect(starts).toEqual([
      [1, 1],
      [3, 1],
    ]);
    expect(nextChange(starts, null)).toEqual([1, 1]);
    expect(nextChange(starts, [1, 1])).toEqual([3, 1]);
    expect(nextChange(starts, [3, 1])).toEqual([1, 1]);
    expect(nextChange(starts, [3, 1], true)).toEqual([1, 1]);
    expect(nextChange(starts, null, true)).toEqual([3, 1]);
    expect(nextChange([], null)).toBeNull();
  });

  it('the rendered view marks the old side’s removed lines and the new side’s added ones', () => {
    const changed = changedLines(parseHunks(PATCH));
    expect([...changed.old]).toEqual([3, 8]);
    expect([...changed.new]).toEqual([3, 8, 9]);
  });

  it('a selection in the rendered view spans the lines whose boxes it crosses', () => {
    const box = (y: number) => ({ x: 0, y, width: 100, height: 20 });
    const drawn = [
      { line: 1, rects: [box(0)] },
      { line: 3, rects: [box(30)] },
      { line: 4, rects: [box(60), box(80)] },
    ];
    expect(linesUnder(drawn, [{ top: 35, bottom: 70 }])).toEqual([3, 4]);
    expect(linesUnder(drawn, [{ top: 200, bottom: 210 }])).toBeNull();
  });

  it('code is fenced longer than its backticks, with its language', () => {
    expect(fenced(['a ```` b'], 'md')).toBe('`````md\na ```` b\n`````\n');
    expect(fenced(['x'], null)).toBe('```\nx\n```\n');
  });

  // ATM-589: an empty last line is a line of the block, so the run is as tall as its rows and nothing is drawn below it
  it('keeps every line, an empty last one too; no lines, no line; one empty line, one', () => {
    // An empty last line: one newline more, as the block's text drops its last and a final newline draws no line
    expect(fenced(['x', ''], null)).toBe('```\nx\n\n\n```\n');
    expect(fenced(['', 'x', '', ''], 'rs')).toBe('```rs\n\nx\n\n\n\n```\n');
    expect(fenced(['', 'x'], 'rs')).toBe('```rs\n\nx\n```\n');
    expect(fenced([], null)).toBe('```\n```\n');
    expect(fenced([''], null)).toBe('```\n\n\n```\n');
  });

  // ATM-589: a hunk whose last line is an empty context line, before the fold that ends the file
  it('a hunk ending with an empty line: its rows are its lines, the fold the rest of the file', () => {
    const now = Array.from({ length: 40 }, (_, i) => (i + 1 === 31 ? '' : `line ${i + 1}`)).join('\n') + '\n';
    const patch = '@@ -28,4 +28,4 @@\n line 28\n-line 29\n+LINE 29\n line 30\n \n';
    const unified = unifiedBlocks(parseHunks(patch), now).find((b) => b.type === 'rows');
    expect(unified?.type === 'rows' && unified.rows.map((r) => [r.old, r.new])).toEqual([
      [28, 28],
      [29, null],
      [null, 29],
      [30, 30],
      [31, 31],
    ]);
    for (const blocks of [splitBlocks(parseHunks(patch), now), unifiedBlocks(parseHunks(patch), now)]) {
      expect(blocks.filter((b) => b.type === 'rows')).toHaveLength(1);
      const folds = blocks.filter((b) => b.type === 'fold');
      expect(folds.map((f) => f.type === 'fold' && [f.fold.newFrom, f.fold.count])).toEqual([
        [1, 27],
        [32, 9],
      ]);
    }
    const split = splitBlocks(parseHunks(patch), now).find((b) => b.type === 'rows');
    expect(split?.type === 'rows' && split.rows.map((r) => [r.old, r.new])).toEqual([
      [28, 28],
      [29, 29],
      [30, 30],
      [31, 31],
    ]);
  });

  it('a file ending with a newline has no empty last row', () => {
    expect(linesOf('a\nb\n')).toEqual(['a', 'b']);
    expect(linesOf('a\r\nb\r\n')).toEqual(['a', 'b']);
    // A file ending with an empty line: that line is the file's
    expect(linesOf('a\n\n')).toEqual(['a', '']);
    expect(linesOf('a')).toEqual(['a']);
    expect(linesOf('')).toEqual([]);
    expect(linesOf(null)).toEqual([]);
  });
});
