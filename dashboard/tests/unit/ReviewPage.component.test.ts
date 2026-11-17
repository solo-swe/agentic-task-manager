import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DocumentResult } from '../../src/lib/documents';
import type { DocumentSource } from '../../src/lib/documents/source';
import ReviewPage from '../../src/lib/reviews/ReviewPage.svelte';
import type { PlacedChain, ReviewPageData, ViewFile } from '../../src/lib/reviews/types';

// A review's page (lib/reviews/ReviewPage.svelte, ATM-574): its data and its sides are given, what the user does is
// reported through the handlers (stubs). CompMD doesn't load in these tests (its fallback text shows).
const loadCompmd = vi.hoisted(() => vi.fn());
vi.mock('../../src/lib/compmd', async (actual) => ({ ...(await actual()), loadCompmd }));

const PATCH = '@@ -1,2 +1,2 @@\n one\n-two\n+TWO\n';

const file = (path: string, more: Partial<ViewFile> = {}): ViewFile => ({
  path,
  old_path: path,
  status: 'modified',
  binary: false,
  patch: PATCH,
  before: 'base1',
  after: 'head2',
  ...more,
});

const chain = (id: string, path: string | null, more: Partial<PlacedChain> = {}): PlacedChain => ({
  id,
  anchor: { round: 1, path, side: 'new', lines: path ? [2, 2] : null, quote: null },
  comments: [
    { id: `c-${id}`, author: 'Ana', at: '2026-11-17T10:00:00Z', body: `Comment ${id}`, resolved: false, replies: [] },
  ],
  placed: path ? [2, 2] : null,
  drifted: null,
  ...more,
});

function data(more: Partial<ReviewPageData> = {}, view: Partial<ReviewPageData['view']> = {}): ReviewPageData {
  return {
    review: {
      id: 'r1',
      reference: 'CR-7',
      number: 7,
      title: 'Trim the names',
      status: 'open',
      tasks: ['ATM-1'],
      author: 'claude:a',
      reviewers: ['Ana'],
      created: '2026-11-17T10:00:00Z',
      updated: '2026-11-17T10:00:00Z',
    },
    summary: 'What changed, **in short**.',
    branch: 'dev/1',
    rounds: [1, 2, 3].map((number) => ({ number, kind: { kind: 'files' }, files: [], by: 'claude:a', at: '' })),
    chains: [],
    view: { round: 3, against: null, files: [file('src/b.rs'), file('README.md'), file('src/a.rs')], ...view },
    needs_you: true,
    you: 'Ana',
    ...more,
  };
}

/** A side: every file it is asked for, as two lines */
const side = (revision: string): DocumentSource => ({
  id: revision,
  load: async (path): Promise<DocumentResult> => ({
    unchanged: false,
    document: {
      path,
      kind: 'code',
      language: 'text',
      text: revision.startsWith('base') ? 'one\ntwo\n' : 'one\nTWO\n',
      document: '',
      line_offset: 1,
      lines: 2,
      bytes: 8,
      modified_at: null,
      tracked: true,
      commit: null,
      submodule: null,
    },
    validators: {},
  }),
  imageUrl: () => null,
  repositoryLink: () => null,
});

function show(page: ReviewPageData = data(), more: Record<string, unknown> = {}) {
  const handlers = {
    onfile: vi.fn(),
    onround: vi.fn(),
    onviewed: vi.fn(),
    oncomment: vi.fn(),
    onreply: vi.fn(),
    onresolve: vi.fn(),
    onsubmit: vi.fn(),
  };
  const sides = vi.fn(side);
  const result = render(ReviewPage, {
    page,
    side: sides,
    isViewed: (f: ViewFile) => f.path === 'README.md',
    taskHref: (key: string) => `/projects/p1/list?task=${key}`,
    ...handlers,
    ...more,
  });
  return { ...result, ...handlers, sides };
}

beforeEach(() => {
  vi.spyOn(console, 'error').mockImplementation(() => {});
  loadCompmd.mockRejectedValue(new Error('no CompMD in these tests'));
  Element.prototype.scrollIntoView = vi.fn();
});
afterEach(() => vi.restoreAllMocks());

describe('ReviewPage: the header', () => {
  it('shows the reference, title, status, who decides, the branch, the tasks as links, the summary', () => {
    const { container } = show();
    const header = within(container.querySelector('.review-header') as HTMLElement);
    expect(header.getByRole('heading').textContent).toContain('CR-7');
    expect(header.getByRole('heading').textContent).toContain('Trim the names');
    expect(header.getByText('Open')).toBeTruthy();
    expect(header.getByText('Needs you')).toBeTruthy();
    expect(header.getByText('by claude:a')).toBeTruthy();
    expect(header.getByText('decided by Ana')).toBeTruthy();
    expect(header.getByText('dev/1')).toBeTruthy();
    expect(header.getByRole('link', { name: 'ATM-1' }).getAttribute('href')).toBe('/projects/p1/list?task=ATM-1');
    expect(container.querySelector('.review-summary')).not.toBeNull();
  });

  it('says who decides when it names nobody, and that it has no author yet; no needs-you mark, no summary, tasks as text without links', () => {
    const page = data({ needs_you: false, summary: '  ', branch: null });
    page.review = { ...page.review, author: null, reviewers: [] };
    const { container } = show(page, { taskHref: undefined });
    expect(screen.getByText('by nobody yet')).toBeTruthy();
    expect(screen.getByText('decided by anyone but its author')).toBeTruthy();
    expect(screen.queryByText('Needs you')).toBeNull();
    expect(container.querySelector('.review-summary')).toBeNull();
    expect(screen.queryByRole('link', { name: 'ATM-1' })).toBeNull();
    expect(screen.getByText('ATM-1')).toBeTruthy();
  });

  it('shows a problem as an alert', () => {
    show(data(), { problem: "the review's author can't decide it" });
    expect(screen.getByRole('alert').textContent).toBe("the review's author can't decide it");
  });
});

describe('ReviewPage: the rounds', () => {
  it('picks a round (its whole change), and a round to compare it against: only earlier ones', async () => {
    const { onround } = show();
    const round = screen.getByLabelText('Round') as HTMLSelectElement;
    const against = screen.getByLabelText('Showing') as HTMLSelectElement;
    expect([...round.options].map((o) => o.textContent)).toEqual(['1 of 3', '2 of 3', '3 of 3']);
    expect(round.value).toBe('3');
    expect([...against.options].map((o) => o.value)).toEqual(['', '1', '2']);
    await fireEvent.change(round, { target: { value: '2' } });
    await fireEvent.change(against, { target: { value: '1' } });
    await fireEvent.change(against, { target: { value: '' } });
    expect(onround.mock.calls).toEqual([
      [2, null],
      [3, 1],
      [3, null],
    ]);
  });

  it('round 1 has nothing to compare against', () => {
    show(data({}, { round: 1 }));
    expect([...(screen.getByLabelText('Showing') as HTMLSelectElement).options].map((o) => o.value)).toEqual(['']);
  });
});

describe('ReviewPage: the files', () => {
  it('lists them as a tree, each with how it changed and its viewed tick, and counts the ones viewed', () => {
    const { container } = show(
      data({}, { files: [file('src/b.rs'), file('README.md'), file('src/new.rs', { status: 'added', before: null })] }),
    );
    const rows = [...container.querySelectorAll('.review-files li')].map((li) =>
      li.textContent?.replace(/\s+/g, ' ').trim(),
    );
    expect(rows).toEqual(['src/', 'M b.rs +1 −1', 'A new.rs +1 −1', 'M README.md +1 −1']);
    expect((screen.getByLabelText('Viewed README.md') as HTMLInputElement).checked).toBe(true);
    expect((screen.getByLabelText('Viewed src/b.rs') as HTMLInputElement).checked).toBe(false);
    expect(screen.getByText('1 of 3 viewed')).toBeTruthy();
  });

  it('shows the file the address names, else the first; a click shows another, a tick marks it viewed', async () => {
    const { container, onfile, onviewed } = show(data(), { file: 'src/a.rs' });
    expect(container.querySelector('.review-file.current [data-file]')?.getAttribute('data-file')).toBe('src/a.rs');
    await fireEvent.click(screen.getByRole('button', { name: /README\.md/ }));
    expect(onfile).toHaveBeenCalledWith('README.md');
    await fireEvent.click(screen.getByLabelText('Viewed src/b.rs'));
    expect(onviewed).toHaveBeenCalledWith(expect.objectContaining({ path: 'src/b.rs' }));
    const other = show(data(), { file: 'gone.rs' });
    expect(other.container.querySelector('.review-file.current [data-file]')?.getAttribute('data-file')).toBe(
      'src/b.rs',
    );
  });

  it('reads each side of the file shown at its revision; a side the file doesn’t have isn’t read', async () => {
    const { sides, container } = show(
      data({}, { files: [file('new.rs', { status: 'added', before: null, after: 'head2' })] }),
    );
    await vi.waitFor(() => expect(container.querySelector('.diff-view')).not.toBeNull());
    expect(sides.mock.calls.map(([r]) => r)).toEqual(['head2']);
  });

  it('a binary file is listed, not shown; a view with no files says so', () => {
    const binary = show(data({}, { files: [file('logo.png', { binary: true, patch: '' })] }));
    expect(screen.getByText('logo.png is a binary file: listed, not shown.')).toBeTruthy();
    expect(binary.container.querySelector('.diff-view')).toBeNull();
    expect(screen.getByText('binary')).toBeTruthy();
    binary.unmount();
    show(data({}, { files: [] }));
    expect(screen.getByText('No files changed here.')).toBeTruthy();
  });

  it('] and [ show the next and previous file, none past either end', async () => {
    const { onfile, container } = show(data(), { file: 'README.md' });
    await vi.waitFor(() => expect(container.querySelector('.diff-view')).not.toBeNull());
    await fireEvent.keyDown(window, { key: ']' });
    await fireEvent.keyDown(window, { key: '[' });
    expect(onfile.mock.calls).toEqual([['src/a.rs'], ['src/b.rs']]);
  });
});

describe('ReviewPage: the chains', () => {
  it('a drifted chain links to where it was made', async () => {
    const drifted = chain('t2', 'src/b.rs', { drifted: { round: 1, lines: [7, 8] } });
    const { onround } = show(data({ chains: [drifted] }));
    const link = screen.getByRole('button', { name: /see it where it was made, round 1, lines 7–8/ });
    await fireEvent.click(link);
    expect(onround).toHaveBeenCalledWith(1, null);
  });

  it('the review’s own chains show under the diff, with a comment of one’s own on the review', async () => {
    const { oncomment, container } = show(data({ chains: [chain('t3', null)] }));
    // Its text is CompMD's to draw (task text, I1): the chain is there with its comment
    expect(container.querySelector('.review-conversation [data-comment="c-t3"]')).not.toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: 'Comment on the review' }));
    expect(screen.getByText('The review as a whole')).toBeTruthy();
    const save = screen.getAllByRole('button', { name: 'Comment' })[0] as HTMLButtonElement;
    expect(save.disabled).toBe(true);
    await fireEvent.input(screen.getByLabelText('Comment'), { target: { value: '  Looks good.  ' } });
    await fireEvent.click(save);
    expect(oncomment).toHaveBeenCalledWith({ path: null, picked: null, body: 'Looks good.' });
    expect(screen.queryByText('The review as a whole')).toBeNull();
  });

  it('a comment started can be cancelled, sending nothing', async () => {
    const { oncomment } = show();
    await fireEvent.click(screen.getByRole('button', { name: 'Comment on the review' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(screen.queryByText('The review as a whole')).toBeNull();
    expect(oncomment).not.toHaveBeenCalled();
  });
});

describe('ReviewPage: submit', () => {
  it('approves or requests changes, with or without a summary; comments only with one', async () => {
    const { onsubmit } = show();
    const comment = screen.getByRole('button', { name: 'Comment' }) as HTMLButtonElement;
    expect(comment.disabled).toBe(true);
    await fireEvent.click(screen.getByRole('button', { name: 'Approve' }));
    await fireEvent.input(screen.getByLabelText('Summary'), { target: { value: ' Two things. ' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Request changes' }));
    await fireEvent.click(comment);
    expect(onsubmit.mock.calls).toEqual([
      ['approve', ''],
      ['request_changes', 'Two things.'],
      ['comment', 'Two things.'],
    ]);
  });

  it('an ended review takes no decision; a busy page none until it is done', () => {
    for (const status of ['approved', 'closed'] as const) {
      const page = data();
      page.review = { ...page.review, status };
      const { unmount } = show(page);
      expect(screen.queryByRole('button', { name: 'Approve' })).toBeNull();
      unmount();
    }
    show(data(), { busy: true });
    expect((screen.getByRole('button', { name: 'Approve' }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('button', { name: 'Request changes' }) as HTMLButtonElement).disabled).toBe(true);
  });
});
