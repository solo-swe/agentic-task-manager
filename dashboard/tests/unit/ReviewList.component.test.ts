import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import ReviewList from '../../src/lib/reviews/ReviewList.svelte';
import { NO_QUERY, type ListQuery, type ReviewEntry, type ReviewStatus } from '../../src/lib/reviews/types';

// The reviews page (lib/reviews/ReviewList.svelte, ATM-574): the list and the query are given; what the user does is
// reported through the handlers, which are stubs here

const entry = (
  n: number,
  status: ReviewStatus,
  more: Partial<ReviewEntry['review']> = {},
  extra: Partial<ReviewEntry> = {},
): ReviewEntry => ({
  review: {
    id: `r${n}`,
    reference: `CR-${n}`,
    number: n,
    title: `Review ${n}`,
    status,
    tasks: [],
    author: 'claude:a',
    reviewers: [],
    created: '2026-11-17T10:00:00Z',
    updated: '2026-11-17T10:00:00Z',
    ...more,
  },
  branch: 'main',
  chains: 2,
  open: 1,
  needs_you: false,
  ...extra,
});

function show(entries: ReviewEntry[], query: ListQuery = { ...NO_QUERY }, more: Record<string, unknown> = {}) {
  const onquery = vi.fn();
  const onopen = vi.fn();
  const result = render(ReviewList, {
    entries,
    query,
    href: (r: string) => `/projects/p1/reviews/${r}`,
    onquery,
    onopen,
    ...more,
  });
  return { ...result, onquery, onopen };
}

beforeEach(() => vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] }));
afterEach(() => vi.useRealTimers());

describe('ReviewList', () => {
  it('lists each review: its reference, title, status, tasks, branch, author, open chains', () => {
    const { container } = show([
      entry(2, 'waiting', { tasks: ['ATM-1', 'ATM-2'] }, { branch: 'dev/1' }),
      entry(1, 'approved', { author: null }, { branch: null }),
    ]);
    const rows = [...container.querySelectorAll('.review-row')];
    expect(rows.map((r) => r.getAttribute('data-review'))).toEqual(['CR-2', 'CR-1']);
    const first = within(rows[0] as HTMLElement);
    expect(first.getByRole('link').getAttribute('href')).toBe('/projects/p1/reviews/CR-2');
    expect(first.getByText('Review 2')).toBeTruthy();
    expect(first.getByText('Waiting for review')).toBeTruthy();
    expect(first.getByText('ATM-1, ATM-2')).toBeTruthy();
    expect(first.getByText('dev/1')).toBeTruthy();
    expect(first.getByText('1 open of 2')).toBeTruthy();
    expect(within(rows[1] as HTMLElement).getByText('no author yet')).toBeTruthy();
    expect(within(rows[1] as HTMLElement).getByText('Approved')).toBeTruthy();
  });

  it('marks those that need you, and offers the first of them next', async () => {
    const { container, onopen } = show([
      entry(3, 'waiting'),
      entry(2, 'changes_requested', {}, { needs_you: true }),
      entry(1, 'live', {}, { needs_you: true }),
    ]);
    expect([...container.querySelectorAll('.needs-you')].map((r) => r.getAttribute('data-review'))).toEqual([
      'CR-2',
      'CR-1',
    ]);
    const next = screen.getByRole('button', { name: /Next review needing you/ });
    expect(next.textContent).toContain('2');
    await fireEvent.click(next);
    expect(onopen).toHaveBeenCalledWith('CR-2', false);
  });

  it('offers no next review when none needs you', () => {
    show([entry(1, 'waiting')]);
    expect(screen.queryByRole('button', { name: /Next review needing you/ })).toBeNull();
  });

  it('opens a review on a click, in a new tab with ⌘ or Ctrl; leaves Shift, Alt and other buttons to the browser', async () => {
    const { onopen } = show([entry(1, 'waiting')]);
    const link = screen.getByRole('link');
    await fireEvent.click(link);
    await fireEvent.click(link, { metaKey: true });
    await fireEvent.click(link, { ctrlKey: true });
    expect(onopen.mock.calls).toEqual([
      ['CR-1', false],
      ['CR-1', true],
      ['CR-1', true],
    ]);
    onopen.mockClear();
    await fireEvent.click(link, { shiftKey: true });
    await fireEvent.click(link, { altKey: true });
    await fireEvent.click(link, { button: 1 });
    expect(onopen).not.toHaveBeenCalled();
  });

  it('asks for a search once typing stops, trimmed', async () => {
    const { onquery } = show([entry(1, 'waiting')]);
    const search = screen.getByRole('searchbox', { name: 'Search reviews' });
    await fireEvent.input(search, { target: { value: 'tw' } });
    await fireEvent.input(search, { target: { value: ' two words ' } });
    expect(onquery).not.toHaveBeenCalled();
    vi.advanceTimersByTime(300);
    expect(onquery).toHaveBeenCalledTimes(1);
    expect(onquery).toHaveBeenCalledWith({ ...NO_QUERY, q: 'two words' });
  });

  // ATM-613: Open is every review not decided yet, the first chip; the statuses narrow it further
  it('Open comes first, standing for every review not decided yet, before each status', () => {
    show([]);
    const chips = [...document.querySelectorAll('.review-chip')];
    expect(chips.map((c) => [c.getAttribute('data-status'), c.textContent])).toEqual([
      ['open', 'Open'],
      ['draft', 'Draft'],
      ['waiting', 'Waiting for review'],
      ['live', 'Live'],
      ['changes_requested', 'Changes requested'],
      ['approved', 'Approved'],
      ['closed', 'Closed'],
    ]);
    expect(chips[0].getAttribute('title')).toMatch(/not decided yet/);
  });

  it('a status chip adds the status to the query, or takes it out; the others stay', async () => {
    const { onquery } = show([entry(1, 'waiting')], { ...NO_QUERY, status: ['open'], author: 'Ana' });
    expect(screen.getByRole('button', { name: 'Open' }).getAttribute('aria-pressed')).toBe('true');
    await fireEvent.click(screen.getByRole('button', { name: 'Live' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Open' }));
    expect(onquery.mock.calls).toEqual([
      [{ ...NO_QUERY, status: ['open', 'live'], author: 'Ana' }],
      [{ ...NO_QUERY, status: [], author: 'Ana' }],
    ]);
  });

  it('each filter offers what the reviews listed have, and what is chosen', async () => {
    const { onquery } = show(
      [
        entry(2, 'waiting', { author: 'claude:b', reviewers: ['Ana', 'Bo'], tasks: ['ATM-2'] }, { branch: 'dev/2' }),
        entry(1, 'waiting', { author: 'claude:a', reviewers: ['Ana'], tasks: ['ATM-1'] }, { branch: null }),
      ],
      { ...NO_QUERY, branch: 'gone' },
    );
    const options = (name: string) =>
      [...(screen.getByRole('combobox', { name }) as HTMLSelectElement).options].map((o) => o.value);
    expect(options('Author')).toEqual(['', 'claude:a', 'claude:b']);
    expect(options('Reviewer')).toEqual(['', 'Ana', 'Bo']);
    expect(options('Task')).toEqual(['', 'ATM-1', 'ATM-2']);
    // A chosen branch no review listed is on stays a choice
    expect(options('Branch')).toEqual(['', 'dev/2', 'gone']);
    await fireEvent.change(screen.getByRole('combobox', { name: 'Reviewer' }), { target: { value: 'Bo' } });
    expect(onquery).toHaveBeenCalledWith({ ...NO_QUERY, branch: 'gone', reviewer: 'Bo' });
  });

  it('clears every filter, offered only while one is set', async () => {
    show([entry(1, 'waiting')]);
    expect(screen.queryByRole('button', { name: 'Clear' })).toBeNull();
    const { onquery } = show([entry(1, 'waiting')], { ...NO_QUERY, task: 'ATM-1' });
    await fireEvent.click(screen.getByRole('button', { name: 'Clear' }));
    expect(onquery).toHaveBeenCalledWith(NO_QUERY);
  });

  it('says why it is empty: loading, nothing fits, no reviews yet; or the problem', () => {
    const { unmount } = show([], { ...NO_QUERY }, { loading: true });
    expect(screen.getByText('Loading…')).toBeTruthy();
    unmount();
    const fits = show([], { ...NO_QUERY, q: 'x' });
    expect(screen.getByText('No review fits.')).toBeTruthy();
    fits.unmount();
    const none = show([]);
    expect(screen.getByText(/No reviews yet/)).toBeTruthy();
    none.unmount();
    show([entry(1, 'waiting')], { ...NO_QUERY }, { problem: 'no review CR-9' });
    expect(screen.getByRole('alert').textContent).toBe('no review CR-9');
    expect(screen.queryByRole('link')).toBeNull();
  });

  describe('New live review (ATM-576)', () => {
    const form = () => within(screen.getByRole('form', { name: 'New live review' }));
    it('opens a form: a title, a task, the commit it is against; Start sends them, trimmed; done, it closes', async () => {
      const onlive = vi.fn(async () => null);
      show([], { ...NO_QUERY }, { onlive });
      const toggle = screen.getByRole('button', { name: /New live review/ });
      await fireEvent.click(toggle);
      expect(toggle).toHaveAttribute('aria-expanded', 'true');
      expect(form().getByLabelText('Title')).toHaveValue('Live review');
      await fireEvent.input(form().getByLabelText('Task'), { target: { value: ' ATM-12 ' } });
      await fireEvent.input(form().getByLabelText('Against the commit'), { target: { value: 'main' } });
      await fireEvent.click(screen.getByRole('button', { name: 'Start' }));
      await vi.waitFor(() => expect(screen.queryByRole('form', { name: 'New live review' })).toBeNull());
      expect(onlive).toHaveBeenCalledWith({ title: 'Live review', base: 'main', task: 'ATM-12' });
    });

    it('refused, it says why and keeps what was written; no title, nothing to start; Cancel closes it', async () => {
      const onlive = vi.fn(async () => 'ATM-99: no such task');
      show([], { ...NO_QUERY }, { onlive });
      await fireEvent.click(screen.getByRole('button', { name: /New live review/ }));
      await fireEvent.input(form().getByLabelText('Task'), { target: { value: 'ATM-99' } });
      await fireEvent.click(screen.getByRole('button', { name: 'Start' }));
      expect(await screen.findByRole('alert')).toHaveTextContent('ATM-99: no such task');
      expect(form().getByLabelText('Task')).toHaveValue('ATM-99');
      await fireEvent.input(form().getByLabelText('Title'), { target: { value: '  ' } });
      expect(screen.getByRole('button', { name: 'Start' })).toBeDisabled();
      await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
      expect(screen.queryByRole('form', { name: 'New live review' })).toBeNull();
      expect(onlive).toHaveBeenCalledTimes(1);
    });

    it('not offered without a way to open one', () => {
      show([]);
      expect(screen.queryByRole('button', { name: /New live review/ })).toBeNull();
    });
  });
});
