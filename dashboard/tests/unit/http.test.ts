import { afterEach, describe, expect, it } from 'vitest';
import { reviewUrl, reviewsUrl, v1DocumentAtUrl } from '../../src/lib/api/http';
import { setApiWorkspace } from '../../src/lib/api/workspace';

// The review routes' addresses (lib/api/http.ts, ATM-574): the current workspace's, `default`'s the v1 ones
describe('the review routes’ addresses', () => {
  afterEach(() => setApiWorkspace('default'));

  it('a project’s reviews: every filter given, status repeated, none empty', () => {
    expect(reviewsUrl('p1')).toBe('/api/v1/projects/p1/reviews');
    expect(reviewsUrl('p1', { q: '', status: [], author: '' })).toBe('/api/v1/projects/p1/reviews');
    expect(
      reviewsUrl('p 1', {
        q: 'two words',
        status: ['open', 'live'],
        author: 'claude:a',
        reviewer: 'Ana',
        task: 'ATM-1',
        branch: 'dev/1',
      }),
    ).toBe(
      '/api/v1/projects/p%201/reviews?q=two+words&status=open&status=live&author=claude%3Aa&reviewer=Ana&task=ATM-1&branch=dev%2F1',
    );
    setApiWorkspace('my work');
    expect(reviewsUrl('p1', { task: 'T-1' })).toBe('/api/v2/workspaces/my%20work/projects/p1/reviews?task=T-1');
  });

  it('a review’s page: its reference, and the round and the round against when given', () => {
    expect(reviewUrl('p1', 'CR-7')).toBe('/api/v1/projects/p1/reviews/CR-7');
    expect(reviewUrl('p1', 'CR-7', 3)).toBe('/api/v1/projects/p1/reviews/CR-7?round=3');
    expect(reviewUrl('p1', 'CR-7', 3, 1)).toBe('/api/v1/projects/p1/reviews/CR-7?round=3&against=1');
    expect(reviewUrl('p1', 'CR-7', null, 1)).toBe('/api/v1/projects/p1/reviews/CR-7?against=1');
    expect(reviewUrl('p1', 'CR 8/2', null, null)).toBe('/api/v1/projects/p1/reviews/CR%208%2F2');
    setApiWorkspace('w');
    expect(reviewUrl('p1', 'CR-7', 2)).toBe('/api/v2/workspaces/w/projects/p1/reviews/CR-7?round=2');
  });

  it('a file at a revision: the documents route with the revision', () => {
    expect(v1DocumentAtUrl('p1', 'docs/a b.md', '3fa1')).toBe(
      '/api/v1/projects/p1/documents?path=docs%2Fa%20b.md&at=3fa1',
    );
    expect(v1DocumentAtUrl('p1', 'a.md', 'HEAD~1')).toBe('/api/v1/projects/p1/documents?path=a.md&at=HEAD~1');
  });
});
