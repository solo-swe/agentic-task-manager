import { fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import ReviewSearch from '../../src/lib/reviews/ReviewSearch.svelte';
import { offerReviewSearch, offeredReviewSearch } from '../../src/lib/reviews/search.svelte';

// The reviews search in the title bar (lib/reviews/ReviewSearch.svelte, ATM-738), and the offer it shows
// (search.svelte.ts): what is typed goes to the offered search a moment after typing stops

function show(q = '') {
  const search = vi.fn();
  const result = render(ReviewSearch, { offered: { q, search } });
  return { ...result, search, box: screen.getByRole('searchbox', { name: 'Search reviews' }) as HTMLInputElement };
}

beforeEach(() => vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] }));
afterEach(() => vi.useRealTimers());

describe('ReviewSearch', () => {
  it('shows the query’s text', () => {
    expect(show('names').box.value).toBe('names');
  });

  it('searches once typing stops, trimmed', async () => {
    const { search, box } = show();
    await fireEvent.input(box, { target: { value: 'tw' } });
    await fireEvent.input(box, { target: { value: ' two words ' } });
    expect(search).not.toHaveBeenCalled();
    vi.advanceTimersByTime(300);
    expect(search.mock.calls).toEqual([['two words']]);
  });

  it('× clears it at once, and a search still waiting is dropped', async () => {
    const { search, box } = show('names');
    await fireEvent.input(box, { target: { value: 'names and' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Clear search' }));
    vi.advanceTimersByTime(300);
    expect(search.mock.calls).toEqual([['']]);
    expect(box.value).toBe('');
    expect(document.activeElement).toBe(box);
  });

  it('"/" focuses it, unless typing somewhere already; ⌘/ and Ctrl+/ don’t', async () => {
    const { box } = show();
    const other = document.body.appendChild(document.createElement('input'));
    other.focus();
    await fireEvent.keyDown(other, { key: '/' });
    expect(document.activeElement).toBe(other);
    other.blur();
    await fireEvent.keyDown(document.body, { key: '/', metaKey: true });
    await fireEvent.keyDown(document.body, { key: '/', ctrlKey: true });
    expect(document.activeElement).not.toBe(box);
    await fireEvent.keyDown(document.body, { key: '/' });
    expect(document.activeElement).toBe(box);
    other.remove();
  });
});

describe('the offered reviews search', () => {
  it('is the last offered; taking one back leaves another’s', () => {
    const one = { q: '', search: vi.fn() };
    const two = { q: '', search: vi.fn() };
    const takeOne = offerReviewSearch(one);
    expect(offeredReviewSearch()).toBe(one);
    const takeTwo = offerReviewSearch(two);
    takeOne();
    expect(offeredReviewSearch()).toBe(two);
    takeTwo();
    expect(offeredReviewSearch()).toBeNull();
  });
});
