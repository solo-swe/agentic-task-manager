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

  // ATM-593: a row's task keys are links to their tasks, those that are tasks; a click opens the task, not the review
  it('a row’s tasks: a known key a link that opens its task (⌘ a tab of its own), an unknown one text', async () => {
    const open = vi.fn();
    const { onopen, container } = show(
      [entry(2, 'waiting', { tasks: ['ATM-1', 'ATM-2'] })],
      { ...NO_QUERY },
      {
        taskHref: (key: string) => `/projects/p1/list?task=${key}`,
        taskLinks: { known: (k: string) => k === 'ATM-1', open },
      },
    );
    const link = screen.getByRole('link', { name: 'ATM-1' });
    expect(link).toHaveAttribute('href', '/projects/p1/list?task=ATM-1');
    expect(screen.queryByRole('link', { name: 'ATM-2' })).toBeNull();
    expect(container.querySelector('.review-tasks')).toHaveTextContent('ATM-1, ATM-2');
    await fireEvent.click(link);
    await fireEvent.click(link, { metaKey: true });
    expect(open.mock.calls).toEqual([
      ['ATM-1', false],
      ['ATM-1', true],
    ]);
    expect(onopen).not.toHaveBeenCalled();
  });

  it('marks those that need you, and offers the first of them shown next', async () => {
    const { container, onopen } = show([
      entry(3, 'waiting'),
      entry(2, 'changes_requested', {}, { needs_you: true }),
      entry(1, 'live', {}, { needs_you: true }),
    ]);
    // In groups (ATM-610): CR-1 waits for your review, so it is shown first; CR-2, sent back with open comments, after
    expect([...container.querySelectorAll('.needs-you')].map((r) => r.getAttribute('data-review'))).toEqual([
      'CR-1',
      'CR-2',
    ]);
    const next = screen.getByRole('button', { name: /Next review needing you/ });
    expect(next.textContent).toContain('2');
    await fireEvent.click(next);
    expect(onopen).toHaveBeenCalledWith('CR-1', false, expect.objectContaining({ needs_you: true }));
  });

  // ATM-575: on the overview's list of every project's reviews, each row says its project, and its link is its own
  it('a row of another project’s list says its project, and its link and opening carry the row', async () => {
    const theirs = { ...entry(1, 'waiting'), project: { id: 'p2', name: 'CompMD' } };
    const href = vi.fn((reference: string, e?: ReviewEntry) => `/projects/${e?.project?.id}/reviews/${reference}`);
    const { container, onopen } = show([theirs], { ...NO_QUERY }, { href });
    expect(container.querySelector('.review-project')).toHaveTextContent('CompMD');
    expect(screen.getByRole('link')).toHaveAttribute('href', '/projects/p2/reviews/CR-1');
    await fireEvent.click(screen.getByRole('link'));
    expect(onopen).toHaveBeenCalledWith('CR-1', false, theirs);
  });

  // ATM-610: unfiltered, the list in groups, each under its heading with its count; any filter, the plain list
  describe('in groups', () => {
    const now = Date.now();
    const ago = (days: number) => new Date(now - days * 86_400_000).toISOString();
    const groupsOf = (container: HTMLElement) =>
      [...container.querySelectorAll('.review-group')].map((g) => [
        g.getAttribute('data-group'),
        g.querySelector('.review-group-count')?.textContent,
        [...g.querySelectorAll('.review-row')].map((r) => r.getAttribute('data-review')),
      ]);

    beforeEach(() => localStorage.clear());

    it('each group under its heading and count; waiting for you says how long; Older folded until opened', async () => {
      const { container } = show([
        entry(1, 'waiting', {}, { needs_you: true, open: 0, waiting_since: ago(3) }),
        entry(2, 'approved', { updated: ago(1) }, { open: 0 }),
        entry(3, 'closed', { updated: ago(30) }, { open: 0 }),
      ]);
      expect(groupsOf(container)).toEqual([
        ['waiting', '1', ['CR-1']],
        ['recent', '1', ['CR-2']],
        ['older', '1', []],
      ]);
      expect(screen.getByText('Waiting for your review')).toBeTruthy();
      expect(screen.getByText('Waited 3 days')).toBeTruthy();
      const fold = screen.getByRole('button', { name: /Older/ });
      expect(fold).toHaveAttribute('aria-expanded', 'false');
      await fireEvent.click(fold);
      expect(groupsOf(container).at(-1)).toEqual(['older', '1', ['CR-3']]);
      expect(JSON.parse(localStorage.getItem('atm-review-groups') ?? '{}')).toEqual({ olderOpen: true });
    });

    it('Older opened before stays open', () => {
      localStorage.setItem('atm-review-groups', JSON.stringify({ olderOpen: true }));
      const { container } = show([entry(3, 'closed', { updated: ago(30) }, { open: 0 })]);
      expect(groupsOf(container)).toEqual([['older', '1', ['CR-3']]]);
    });

    it('a filter or a search shows the plain list, no groups', () => {
      const entries = [
        entry(2, 'approved', { updated: ago(1) }, { open: 0 }),
        entry(1, 'waiting', {}, { needs_you: true, open: 0 }),
      ];
      const { container } = show(entries, { ...NO_QUERY, q: 'x' });
      expect(container.querySelector('.review-group')).toBeNull();
      expect([...container.querySelectorAll('.review-row')].map((r) => r.getAttribute('data-review'))).toEqual([
        'CR-2',
        'CR-1',
      ]);
      expect(screen.queryByText(/Waited/)).toBeNull();
    });
  });

  it('offers no next review when none needs you', () => {
    show([entry(1, 'waiting')]);
    expect(screen.queryByRole('button', { name: /Next review needing you/ })).toBeNull();
  });

  it('opens a review on a click, in a new tab with ⌘ or Ctrl; leaves Shift, Alt and other buttons to the browser', async () => {
    const one = entry(1, 'waiting');
    const { onopen } = show([one]);
    const link = screen.getByRole('link');
    await fireEvent.click(link);
    await fireEvent.click(link, { metaKey: true });
    await fireEvent.click(link, { ctrlKey: true });
    // With its row (ATM-575: on a list of several projects' reviews, a reference names one in each)
    expect(onopen.mock.calls).toEqual([
      ['CR-1', false, one],
      ['CR-1', true, one],
      ['CR-1', true, one],
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

  // ATM-617: a review atm can't read is a row of its own, with why and no link, after the others; alone, it is no
  // empty list
  it('lists a review atm can’t read as a row of its own, after the others: its reference, why, no link', () => {
    const unreadable = [{ folder: 'CR-13-3b8e', reference: 'CR-13', problem: 'no status frozen' }];
    const { container, unmount } = show([entry(2, 'waiting')], { ...NO_QUERY }, { unreadable });
    const rows = [...container.querySelectorAll('.review-row')];
    expect(rows.map((r) => r.getAttribute('data-review') ?? r.getAttribute('data-unreadable'))).toEqual([
      'CR-2',
      'CR-13',
    ]);
    const row = within(rows[1] as HTMLElement);
    expect(row.getByText("Can't be read")).toBeTruthy();
    expect(row.getByText('no status frozen')).toHaveAttribute('title', 'CR-13-3b8e');
    expect(row.queryByRole('link')).toBeNull();
    unmount();
    show([], { ...NO_QUERY, q: 'x' }, { unreadable });
    expect(screen.queryByText('No review fits.')).toBeNull();
    expect(screen.getByText('CR-13')).toBeTruthy();
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
