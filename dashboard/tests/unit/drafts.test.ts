import { describe, expect, it } from 'vitest';
import { withDrafts } from '../../src/lib/reviews/drafts';
import type { PlacedChain, PlacedDraft, ReviewComment } from '../../src/lib/reviews/types';

// The user's drafts among a review's chains (lib/reviews/drafts.ts, ATM-612): a draft comment a chain of its own,
// placed where the server placed it; a draft reply under the comment it answers.

const comment = (id: string, more: Partial<ReviewComment> = {}): ReviewComment => ({
  id,
  author: 'claude:1',
  at: '2026-11-19T10:00:00Z',
  body: `Body of ${id}`,
  resolved: false,
  replies: [],
  ...more,
});
const chain = (id: string, comments: ReviewComment[]): PlacedChain => ({
  id,
  anchor: { round: 1, path: 'a.rs', side: 'new', lines: [3, 4], quote: null },
  comments,
  placed: [3, 4],
  drifted: null,
  in_view: true,
});
const onLines = (id: string, more: Partial<PlacedDraft> = {}): PlacedDraft =>
  ({
    id,
    at: '2026-11-19T11:00:00Z',
    body: `Draft ${id}`,
    kind: 'comment',
    path: 'b.rs',
    lines: [7, 7],
    side: 'old',
    quote: 'x',
    round: 2,
    placed: [9, 9],
    drifted: null,
    ...more,
  }) as PlacedDraft;
const reply = (id: string, to: string): PlacedDraft => ({
  id,
  at: '2026-11-19T12:00:00Z',
  body: `Reply ${id}`,
  kind: 'reply',
  to,
});

describe('withDrafts', () => {
  it('no drafts (none, or an older atm’s page without them): the chains as they are', () => {
    const chains = [chain('t1', [comment('c1')])];
    expect(withDrafts(chains, undefined, 'Hakan', 2)).toBe(chains);
    expect(withDrafts(chains, [], 'Hakan', 2)).toBe(chains);
  });

  it('a draft comment is a chain of its own after the chains: the user’s, never resolved, marked, where it was placed', () => {
    const [, own] = withDrafts([chain('t1', [comment('c1')])], [onLines('d1')], 'Hakan', 2);
    expect(own).toEqual({
      id: 'draft-d1',
      anchor: { round: 2, path: 'b.rs', side: 'old', lines: [7, 7], quote: 'x' },
      comments: [
        {
          id: 'd1',
          author: 'Hakan',
          at: '2026-11-19T11:00:00Z',
          body: 'Draft d1',
          resolved: null,
          draft: true,
          replies: [],
        },
      ],
      placed: [9, 9],
      drifted: null,
      in_view: true,
    });
    // On a round the view doesn't hold, as the server says (ATM-725)
    const [, elsewhere] = withDrafts([chain('t1', [])], [onLines('d1', { in_view: false })], 'Hakan', 2);
    expect(elsewhere.in_view).toBe(false);
  });

  it('one on what a live review is now is on the round shown; on the review, no file; drifted as the server says', () => {
    const drifted = { round: 1, lines: [2, 2] as [number, number] };
    const chains = withDrafts(
      [],
      [
        onLines('d1', { round: null }),
        onLines('d2', { path: null, lines: null, placed: null }),
        onLines('d3', { drifted }),
      ],
      'Hakan',
      5,
    );
    expect(chains.map((c) => [c.anchor.round, c.anchor.path, c.placed, c.drifted])).toEqual([
      [5, 'b.rs', [9, 9], null],
      [2, null, null, null],
      [2, 'b.rs', [9, 9], drifted],
    ]);
  });

  it('a draft reply goes under the comment it answers, after its replies, at any depth; one whose comment is gone is left out', () => {
    const chains = [
      chain('t1', [comment('c1', { replies: [comment('c2', { resolved: null })] })]),
      chain('t2', [comment('c3')]),
    ];
    const shown = withDrafts(chains, [reply('d1', 'c1'), reply('d2', 'c2'), reply('d3', 'c9')], 'Hakan', 1);
    expect(shown).toHaveLength(2);
    const c1 = shown[0].comments[0];
    expect(c1.replies.map((r) => [r.id, r.draft ?? false])).toEqual([
      ['c2', false],
      ['d1', true],
    ]);
    expect(c1.replies[0].replies.map((r) => r.id)).toEqual(['d2']);
    expect(shown[1]).toEqual(chains[1]);
    // The page's own chains are left as they were
    expect(chains[0].comments[0].replies).toHaveLength(1);
  });
});
