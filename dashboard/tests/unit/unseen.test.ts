import { describe, expect, it } from 'vitest';
import { seenAtFirst, unseenOf, withSeen } from '../../src/lib/reviews/unseen';
import type { Chain, ReviewComment } from '../../src/lib/reviews/types';

// What came on a review since its page opened (lib/reviews/unseen.ts, ATM-733)

const YOU = 'Hakan Serce';
const comment = (id: string, author = 'claude:a', more: Partial<ReviewComment> = {}): ReviewComment => ({
  id,
  author,
  at: '2026-11-24T10:00:00Z',
  body: id,
  resolved: false,
  replies: [],
  ...more,
});
const chain = (id: string, path: string | null, comments: ReviewComment[]): Chain => ({
  id,
  anchor: { round: 1, path, side: 'new', lines: path ? [3, 3] : null, quote: null },
  comments,
});

describe('unseenOf', () => {
  const first = [chain('t1', 'src/a.rs', [comment('c1')]), chain('t2', null, [comment('c2', YOU)])];

  it('nothing is new on the page as it opened', () => {
    const seen = seenAtFirst(first, [1, 2]);
    expect(unseenOf(seen, first, [1, 2], YOU)).toEqual({ comments: new Set(), general: 0, onFiles: 0, round: null });
  });

  it('a reply, a comment on a file and one on the review are new, counted where they are', () => {
    const seen = seenAtFirst(first, [1, 2]);
    const now = [
      chain('t1', 'src/a.rs', [comment('c1', 'claude:a', { replies: [comment('c3')] })]),
      chain('t2', null, [comment('c2', YOU)]),
      chain('t4', 'src/b.rs', [comment('c4')]),
      chain('t5', null, [comment('c5', 'Ana')]),
    ];
    const unseen = unseenOf(seen, now, [1, 2], YOU);
    expect([...unseen.comments].sort()).toEqual(['c3', 'c4', 'c5']);
    expect([unseen.onFiles, unseen.general]).toEqual([2, 1]);
  });

  it('the reader’s own comments and drafts are never new to them', () => {
    const seen = seenAtFirst(first, [1]);
    const now = [
      ...first,
      chain('t6', 'src/a.rs', [comment('c6', YOU)]),
      chain('t7', null, [comment('d1', YOU, { draft: true })]),
      chain('t8', null, [comment('d2', 'claude:a', { draft: true })]),
    ];
    expect(unseenOf(seen, now, [1], YOU).comments.size).toBe(0);
  });

  it('a round past the last one seen is new until it is seen; an older one never is', () => {
    const seen = seenAtFirst(first, [1, 2]);
    expect(unseenOf(seen, first, [1, 2, 3], YOU).round).toBe(3);
    expect(unseenOf({ ...seen, round: 3 }, first, [1, 2, 3], YOU).round).toBeNull();
    expect(unseenOf(seen, first, [1], YOU).round).toBeNull();
    expect(seenAtFirst([], []).round).toBe(0);
  });

  it('a comment seen is no longer new; the others stay', () => {
    const now = [...first, chain('t9', 'x', [comment('c9'), comment('c10')])];
    const seen = withSeen(seenAtFirst(first, [1]), ['c9']);
    expect([...unseenOf(seen, now, [1], YOU).comments]).toEqual(['c10']);
  });
});
