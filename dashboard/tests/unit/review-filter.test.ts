import { beforeEach, describe, expect, it } from 'vitest';
import { REVIEW_FILTER_KEY, keepFilter, keptFilter } from '../../src/lib/review-filter';
import { NO_QUERY, type ListQuery } from '../../src/lib/reviews/types';

// The reviews list's last filter, kept per project (ATM-604)

const query = (more: Partial<ListQuery>): ListQuery => ({ ...NO_QUERY, ...more });
const stored = () => localStorage.getItem(REVIEW_FILTER_KEY);

describe('the reviews list’s kept filter', () => {
  beforeEach(() => localStorage.clear());

  it('is none for a project with none kept', () => {
    expect(keptFilter('p1')).toEqual(NO_QUERY);
  });

  it('is the filter last chosen, each part of it, but not the search words', () => {
    keepFilter('p1', query({ status: ['open'], q: 'words' }));
    keepFilter(
      'p1',
      query({ status: ['open', 'live'], author: 'hakan', reviewer: 'r', task: 'ATM-1', branch: 'dev', q: 'x' }),
    );
    expect(keptFilter('p1')).toEqual(
      query({ status: ['open', 'live'], author: 'hakan', reviewer: 'r', task: 'ATM-1', branch: 'dev' }),
    );
  });

  it('is each project’s own', () => {
    keepFilter('p1', query({ status: ['open'] }));
    keepFilter('p2', query({ status: ['approved'] }));
    expect(keptFilter('p1').status).toEqual(['open']);
    expect(keptFilter('p2').status).toEqual(['approved']);
    expect(keptFilter('p3')).toEqual(NO_QUERY);
  });

  it('is forgotten when cleared, or when only search words are left; the key goes with the last project’s', () => {
    keepFilter('p1', query({ status: ['open'] }));
    keepFilter('p2', query({ task: 'ATM-2' }));
    keepFilter('p1', NO_QUERY);
    expect(keptFilter('p1')).toEqual(NO_QUERY);
    expect(keptFilter('p2').task).toBe('ATM-2');
    keepFilter('p2', query({ q: 'only words' }));
    expect(stored()).toBeNull();
  });

  it('leaves out what was stored that isn’t a filter: unknown statuses, wrong types, a broken value', () => {
    localStorage.setItem(
      REVIEW_FILTER_KEY,
      JSON.stringify({ p1: { status: ['open', 'nope', 3], author: 7, task: 'ATM-1' }, p2: ['open'], p3: 'open' }),
    );
    expect(keptFilter('p1')).toEqual(query({ status: ['open'], task: 'ATM-1' }));
    expect(keptFilter('p2')).toEqual(NO_QUERY);
    expect(keptFilter('p3')).toEqual(NO_QUERY);
    localStorage.setItem(REVIEW_FILTER_KEY, '{broken');
    expect(keptFilter('p1')).toEqual(NO_QUERY);
  });

  it('a filter kept is a copy: changing the query after does not change it', () => {
    const chosen = query({ status: ['open'] });
    keepFilter('p1', chosen);
    chosen.status.push('live');
    expect(keptFilter('p1').status).toEqual(['open']);
  });
});
