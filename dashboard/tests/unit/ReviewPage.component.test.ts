import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DocumentResult } from '../../src/lib/documents';
import type { DocumentSource } from '../../src/lib/documents/source';
import ReviewPage from '../../src/lib/reviews/ReviewPage.svelte';
import type { PlacedChain, ReviewPageData, ViewFile } from '../../src/lib/reviews/types';

// A review's page (lib/reviews/ReviewPage.svelte, ATM-574): its data and its sides are given, what the user does is
// reported through the handlers (stubs). CompMD doesn't load in these tests (its fallback text shows, 20 px a line);
// jsdom has no IntersectionObserver, so every file's diff is made at once.
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

const chain = (id: string, path: string | null, more: Partial<PlacedChain> = {}, resolved = false): PlacedChain => ({
  id,
  anchor: { round: 1, path, side: 'new', lines: path ? [2, 2] : null, quote: null },
  comments: [
    { id: `c-${id}`, author: 'Ana', at: '2026-11-17T10:00:00Z', body: `Comment ${id}`, resolved, replies: [] },
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
      status: 'waiting',
      tasks: ['ATM-1'],
      author: 'claude:a',
      reviewers: ['Ana'],
      created: '2026-11-17T10:00:00Z',
      updated: '2026-11-17T10:00:00Z',
    },
    summary: 'What changed, **in short**.',
    branch: 'dev/1',
    rounds: [1, 2, 3].map((number) => ({
      number,
      kind: { kind: 'files' },
      // Round 2 of commits made in a submodule (ATM-583)
      submodule: number === 2 ? 'lib' : undefined,
      files: [{ path: 'src/a.rs', binary: false }],
      by: 'claude:a',
      at: `2026-11-1${number}`,
    })),
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
      kind: path.endsWith('.md') ? 'markdown' : 'code',
      language: path.endsWith('.md') ? null : 'text',
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
    project: 'ATM',
    side: sides,
    isViewed: (f: ViewFile) => f.path === 'README.md',
    taskHref: (key: string) => `/projects/p1/list?task=${key}`,
    ...handlers,
    ...more,
  });
  return { ...result, ...handlers, sides };
}

const tab = (name: RegExp | string) => screen.getByRole('tab', { name });
const card = (container: HTMLElement, path: string) =>
  container.querySelector(`.review-file-card[data-file="${path}"]`) as HTMLElement;

beforeEach(() => {
  localStorage.clear();
  vi.spyOn(console, 'error').mockImplementation(() => {});
  loadCompmd.mockRejectedValue(new Error('no CompMD in these tests'));
  Element.prototype.scrollIntoView = vi.fn();
});
afterEach(() => vi.restoreAllMocks());

describe('ReviewPage: the top bar', () => {
  it('names the project, the reference and the title; the status, needs-you, and a link back to the reviews', async () => {
    const onback = vi.fn();
    const { container } = show(data(), { backHref: '/projects/p1/reviews', onback });
    const top = within(container.querySelector('.review-top') as HTMLElement);
    expect(top.getByRole('heading').textContent?.replace(/\s+/g, ' ').trim()).toBe('ATM CR-7 Trim the names');
    expect(top.getByText('Waiting for review')).toBeTruthy();
    expect(top.getByText('Needs you')).toBeTruthy();
    const back = top.getByRole('link', { name: 'Reviews' });
    expect(back.querySelector('.icon-arrow-left')).not.toBeNull();
    expect(back.getAttribute('href')).toBe('/projects/p1/reviews');
    await fireEvent.click(back);
    expect(onback).toHaveBeenCalledOnce();
  });

  it('without a project name, a needs-you mark or a way back: the reference and title alone', () => {
    show(data({ needs_you: false }), { project: null });
    expect(screen.getByRole('heading', { level: 1 }).textContent?.replace(/\s+/g, ' ').trim()).toBe(
      'CR-7 Trim the names',
    );
    expect(screen.queryByText('Needs you')).toBeNull();
    expect(screen.queryByRole('link', { name: 'Reviews' })).toBeNull();
  });

  it('shows a problem as an alert', () => {
    show(data(), { problem: "the review's author can't decide it" });
    expect(screen.getByRole('alert').textContent).toBe("the review's author can't decide it");
  });

  it('three tabs, with the files and the comments counted; Overview first, Diffs when the address names a file', async () => {
    const { container, unmount } = show(data({ chains: [chain('t1', 'src/a.rs'), chain('t2', null)] }));
    expect(
      screen
        .getAllByRole('tab')
        .map((t) => [t.querySelector('.review-tab-label')?.textContent, t.querySelector('.review-count')?.textContent]),
    ).toEqual([
      ['Overview', undefined],
      ['Diffs', '3'],
      ['Comments', '2'],
    ]);
    expect(tab('Overview')).toHaveAttribute('aria-selected', 'true');
    expect(container.querySelector('#review-page')).toHaveAttribute('data-tab', 'overview');
    await fireEvent.click(tab(/^Comments/));
    expect(container.querySelector('#review-page')).toHaveAttribute('data-tab', 'comments');
    unmount();
    show(data(), { file: 'src/a.rs' });
    await vi.waitFor(() => expect(tab(/^Diffs/)).toHaveAttribute('aria-selected', 'true'));
  });
});

describe('ReviewPage: Overview', () => {
  it('the facts across the top, the description and the rounds, the conversation beside them', async () => {
    const { container, onround } = show(data({ chains: [chain('t1', 'src/a.rs'), chain('t2', null, {}, true)] }));
    // CompMD's to draw; without it, the text as written
    await vi.waitFor(() => expect(container.querySelector('.review-summary')?.textContent).toContain('What changed'));
    const facts = [...container.querySelectorAll('.review-facts dt')].map((dt) => [
      dt.textContent?.trim(),
      dt.nextElementSibling?.textContent?.replace(/\s+/g, ' ').trim(),
    ]);
    expect(facts).toEqual([
      ['Author', 'claude:a'],
      ['Decided by', 'Ana'],
      ['Branch', 'dev/1'],
      ['Tasks', 'ATM-1'],
      ['Changes', '3 files +3 −3'],
      ['Comments', '1 open · 1 resolved'],
    ]);
    expect(screen.getByRole('link', { name: 'ATM-1' }).getAttribute('href')).toBe('/projects/p1/list?task=ATM-1');
    // The facts first, then the description beside the conversation
    const overview = container.querySelector('.review-overview') as HTMLElement;
    expect(overview.firstElementChild).toHaveClass('review-facts');
    expect(overview.querySelector('.review-overview-body > .review-main .review-summary')).not.toBeNull();
    expect(overview.querySelector('.review-overview-body > .review-conversation [data-chain="t2"]')).not.toBeNull();
    // The rounds, the latest first
    expect(
      [...container.querySelectorAll('.review-round-list li')].map((li) => li.textContent?.replace(/\s+/g, ' ').trim()),
    ).toEqual([
      expect.stringMatching(/^Round 3 1 file · claude:a · .+ shown$/),
      expect.stringMatching(/^Round 2 1 file in the submodule lib · claude:a · /),
      expect.stringMatching(/^Round 1 1 file · claude:a · /),
    ]);
    // A round opens its whole change on the Diffs tab
    await fireEvent.click(screen.getByRole('button', { name: 'Round 2' }));
    expect(onround).toHaveBeenCalledWith(2, null);
    expect(tab(/^Diffs/)).toHaveAttribute('aria-selected', 'true');
  });

  it('says when nobody is named, and that there is no description; no branch, tasks as text without links', () => {
    const page = data({ summary: '  ', branch: null });
    page.review = { ...page.review, author: null, reviewers: [] };
    const { container } = show(page, { taskHref: undefined });
    expect(screen.getByText('No description.')).toBeTruthy();
    expect(screen.getByText('nobody yet')).toBeTruthy();
    expect(screen.getByText('anyone but its author')).toBeTruthy();
    expect([...container.querySelectorAll('.review-facts dt')].map((dt) => dt.textContent)).not.toContain('Branch');
    expect(screen.queryByRole('link', { name: 'ATM-1' })).toBeNull();
    expect(screen.getByText('ATM-1')).toBeTruthy();
  });

  it('the review’s own chains, and a comment of one’s own on it; an empty one isn’t sent, Cancel sends nothing', async () => {
    const { oncomment, container } = show(data({ chains: [chain('t3', null), chain('t4', 'src/a.rs')] }));
    expect(container.querySelector('.review-conversation [data-comment="c-t3"]')).not.toBeNull();
    // A file's chain is the Diffs tab's
    expect(container.querySelector('.review-conversation [data-comment="c-t4"]')).toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: 'Comment on the review' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(screen.queryByLabelText('Comment')).toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: 'Comment on the review' }));
    const save = within(container.querySelector('.review-composer') as HTMLElement).getByRole('button', {
      name: 'Comment',
    });
    expect(save).toBeDisabled();
    await fireEvent.input(screen.getByLabelText('Comment'), { target: { value: '  Looks good.  ' } });
    await fireEvent.click(save);
    expect(oncomment).toHaveBeenCalledOnce();
    expect(oncomment).toHaveBeenCalledWith({ path: null, picked: null, body: 'Looks good.' });
    expect(screen.queryByLabelText('Comment')).toBeNull();
  });
});

describe('ReviewPage: Diffs', () => {
  const diffs = async (page = data(), more: Record<string, unknown> = {}) => {
    const shown = show(page, more);
    await fireEvent.click(tab(/^Diffs/));
    return shown;
  };

  it('picks a round (its whole change), and a round to compare it against: only earlier ones', async () => {
    const { onround } = await diffs();
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

  it('a live review (ATM-576): Now, its working copy, is picked first and says so; a round picked is that round', async () => {
    const live = data({}, { round: 4, now: true });
    live.review = { ...live.review, status: 'live' };
    const { onround, unmount } = await diffs(live);
    const round = screen.getByLabelText('Round') as HTMLSelectElement;
    expect([...round.options].map((o) => o.textContent)).toEqual([
      'Now, as it is being made',
      '1 of 3',
      '2 of 3',
      '3 of 3',
    ]);
    expect(round.value).toBe('now');
    expect(screen.getByText('Following the working copy')).toBeTruthy();
    await fireEvent.change(round, { target: { value: '2' } });
    await fireEvent.change(round, { target: { value: 'now' } });
    expect(onround.mock.calls).toEqual([
      [2, null],
      [null, null],
    ]);
    unmount();
    // A round named: not following
    const named = data({}, { round: 2, now: false });
    named.review = { ...named.review, status: 'live' };
    await diffs(named);
    expect((screen.getByLabelText('Round') as HTMLSelectElement).value).toBe('2');
    expect(screen.queryByText('Following the working copy')).toBeNull();
  });

  it('round 1 has nothing to compare against', async () => {
    await diffs(data({}, { round: 1 }));
    expect([...(screen.getByLabelText('Showing') as HTMLSelectElement).options].map((o) => o.value)).toEqual(['']);
  });

  it('the files as a tree, each with how it changed and its viewed tick; the ones viewed counted', async () => {
    const { container } = await diffs(
      data({}, { files: [file('src/b.rs'), file('README.md'), file('src/new.rs', { status: 'added', before: null })] }),
    );
    const rows = [...container.querySelectorAll('.review-files li')].map((li) =>
      li.textContent?.replace(/\s+/g, ' ').trim(),
    );
    expect(rows).toEqual(['src', 'M b.rs +1 −1', 'A new.rs +1 −1', 'M README.md +1 −1']);
    expect(container.querySelector('.review-folder .icon-folder')).not.toBeNull();
    expect((screen.getAllByLabelText('Viewed README.md')[0] as HTMLInputElement).checked).toBe(true);
    expect((screen.getAllByLabelText('Viewed src/b.rs')[0] as HTMLInputElement).checked).toBe(false);
    expect(screen.getByText('1 of 3 viewed')).toBeTruthy();
  });

  it('every file a card, in the view’s order; a file viewed is folded, its diff not made, its sides not read', async () => {
    const { container, sides } = await diffs();
    expect([...container.querySelectorAll('.review-file-card')].map((c) => (c as HTMLElement).dataset.file)).toEqual([
      'src/b.rs',
      'README.md',
      'src/a.rs',
    ]);
    await vi.waitFor(() => expect(container.querySelectorAll('.diff-view')).toHaveLength(2));
    expect(card(container, 'README.md')).toHaveClass('folded');
    expect(card(container, 'README.md').querySelector('.diff-view')).toBeNull();
    // Each side read at its revision, once per card made
    expect(sides.mock.calls.map(([r]) => r).sort()).toEqual(['base1', 'base1', 'head2', 'head2']);
  });

  it('the fold opens a file viewed, and folds one not; ticking a file viewed folds it again', async () => {
    const { container, onviewed } = await diffs();
    await fireEvent.click(screen.getByRole('button', { name: 'Show README.md' }));
    expect(card(container, 'README.md')).not.toHaveClass('folded');
    await fireEvent.click(screen.getByRole('button', { name: 'Hide src/b.rs' }));
    expect(card(container, 'src/b.rs')).toHaveClass('folded');
    // The card's Viewed: the page is told, and the file follows its tick again
    await fireEvent.click(within(card(container, 'README.md')).getByLabelText('Viewed README.md'));
    expect(onviewed).toHaveBeenCalledWith(expect.objectContaining({ path: 'README.md' }));
    expect(card(container, 'README.md')).toHaveClass('folded');
  });

  it('a click in the tree goes to the file’s card and puts it in the address; a tick there marks it viewed', async () => {
    const { container, onfile, onviewed } = await diffs();
    const scroll = vi.mocked(Element.prototype.scrollIntoView);
    await fireEvent.click(within(container.querySelector('.review-files') as HTMLElement).getByText('a.rs'));
    // The address follows once the page is idle
    expect(onfile).not.toHaveBeenCalled();
    await vi.waitFor(() => expect(onfile).toHaveBeenCalledWith('src/a.rs'));
    expect(scroll.mock.contexts.at(-1)).toBe(card(container, 'src/a.rs'));
    expect(container.querySelector('.review-file.current [data-file]')?.getAttribute('data-file')).toBe('src/a.rs');
    await fireEvent.click(
      within(container.querySelector('.review-files') as HTMLElement).getByLabelText('Viewed src/b.rs'),
    );
    expect(onviewed).toHaveBeenCalledWith(expect.objectContaining({ path: 'src/b.rs' }));
  });

  it('the file gone to stays at the top while the cards around it grow, until the user scrolls', async () => {
    const resized: ResizeObserverCallback[] = [];
    vi.stubGlobal(
      'ResizeObserver',
      class {
        constructor(callback: ResizeObserverCallback) {
          resized.push(callback);
        }
        observe() {}
        disconnect() {}
      },
    );
    try {
      const { container } = await diffs();
      const scroll = vi.mocked(Element.prototype.scrollIntoView);
      await fireEvent.click(within(container.querySelector('.review-files') as HTMLElement).getByText('a.rs'));
      const grow = () => resized.forEach((callback) => callback([], {} as ResizeObserver));
      scroll.mockClear();
      grow();
      expect(scroll.mock.contexts).toContain(card(container, 'src/a.rs'));
      // The user scrolls the list: it stays where they put it
      await fireEvent.wheel(container.querySelector('.review-diffs') as HTMLElement);
      scroll.mockClear();
      grow();
      expect(scroll.mock.contexts).not.toContain(card(container, 'src/a.rs'));
      // Another file gone to is pinned in its turn; Next unpins it
      await fireEvent.keyDown(window, { key: '[' });
      scroll.mockClear();
      grow();
      expect(scroll.mock.contexts).toContain(card(container, 'README.md'));
      await fireEvent.click(screen.getByRole('button', { name: 'Next change' }));
      scroll.mockClear();
      grow();
      expect(scroll.mock.contexts).not.toContain(card(container, 'README.md'));
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it('the address’s file is gone to when the page opens, without writing the address again', async () => {
    const { container, onfile } = show(data(), { file: 'src/a.rs' });
    await vi.waitFor(() =>
      expect(container.querySelector('.review-file.current [data-file]')?.getAttribute('data-file')).toBe('src/a.rs'),
    );
    expect(vi.mocked(Element.prototype.scrollIntoView).mock.contexts).toContain(card(container, 'src/a.rs'));
    expect(onfile).not.toHaveBeenCalled();
  });

  // ATM-595: the tree's folders fold, by click and keys, all at once, kept for the session; a jump opens them
  describe('the tree’s folders fold', () => {
    const treeRows = (container: HTMLElement) =>
      [...container.querySelectorAll('.review-files li')].map((li) => li.textContent?.replace(/\s+/g, ' ').trim());
    beforeEach(() => sessionStorage.clear());

    it('a folder collapses and expands by a click, or ← and →; folded, it sums its files', async () => {
      const { container } = await diffs(data({ chains: [chain('k1', 'src/a.rs')] }));
      const folder = () => container.querySelector('.review-folder[data-folder="src"]') as HTMLElement;
      expect(container.querySelector('[role="tree"]')).not.toBeNull();
      expect(folder()).toHaveAttribute('aria-expanded', 'true');
      await fireEvent.click(screen.getByRole('button', { name: 'Collapse src' }));
      expect(folder()).toHaveAttribute('aria-expanded', 'false');
      // Its two files hidden; the sum says how many, how many have comments, and that none of them is viewed yet
      expect(treeRows(container)).toEqual(['src 2 files · 1 with comments · none viewed', 'M README.md +1 −1']);
      await fireEvent.keyDown(screen.getByRole('button', { name: 'Expand src' }), { key: 'ArrowRight' });
      expect(folder()).toHaveAttribute('aria-expanded', 'true');
      expect(treeRows(container)).toHaveLength(4);
      await fireEvent.keyDown(screen.getByRole('button', { name: 'Collapse src' }), { key: 'ArrowLeft' });
      expect(folder()).toHaveAttribute('aria-expanded', 'false');
      // Other keys are left alone
      await fireEvent.keyDown(screen.getByRole('button', { name: 'Expand src' }), { key: 'ArrowUp' });
      expect(folder()).toHaveAttribute('aria-expanded', 'false');
    });

    it('Collapse all and Expand all; a tree without folders offers neither', async () => {
      const { container, unmount } = await diffs();
      await fireEvent.click(screen.getByRole('button', { name: 'Collapse all folders' }));
      expect(treeRows(container)).toEqual(['src 2 files · none viewed', 'M README.md +1 −1']);
      await fireEvent.click(screen.getByRole('button', { name: 'Expand all folders' }));
      expect(treeRows(container)).toHaveLength(4);
      unmount();
      await diffs(data({}, { files: [file('README.md')] }));
      expect(screen.queryByRole('button', { name: 'Collapse all folders' })).toBeNull();
    });

    it('what is folded is kept for the session, per review; going to a file in a folded folder opens it', async () => {
      const first = await diffs();
      await fireEvent.click(screen.getByRole('button', { name: 'Collapse src' }));
      first.unmount();
      // Opened again: still folded
      const again = await diffs();
      expect(screen.getByRole('button', { name: 'Expand src' })).toBeTruthy();
      again.unmount();
      // Another review: nothing folded
      const other = data();
      other.review = { ...other.review, id: 'another' };
      const elsewhere = await diffs(other);
      expect(screen.getByRole('button', { name: 'Collapse src' })).toBeTruthy();
      elsewhere.unmount();
      // The address names a file in the folded folder: the folder opens, the file marked
      const { container } = show(data(), { file: 'src/a.rs' });
      await vi.waitFor(() =>
        expect(container.querySelector('.review-file.current [data-file]')?.getAttribute('data-file')).toBe('src/a.rs'),
      );
      expect(container.querySelector('.review-folder[data-folder="src"]')).toHaveAttribute('aria-expanded', 'true');
    });
  });

  it('a binary file is listed, not shown; a view with no files says so', async () => {
    const binary = await diffs(data({}, { files: [file('logo.png', { binary: true, patch: '' })] }));
    expect(screen.getByText('A binary file: listed, not shown.')).toBeTruthy();
    expect(binary.container.querySelector('.diff-view')).toBeNull();
    expect(screen.getByText('bin')).toBeTruthy();
    binary.unmount();
    await diffs(data({}, { files: [] }));
    expect(screen.getByText('No files changed here.')).toBeTruthy();
  });

  it('a drifted chain links to where it was made', async () => {
    const drifted = chain('t2', 'src/b.rs', { drifted: { round: 1, lines: [7, 8] }, placed: null });
    const { onround } = await diffs(data({ chains: [drifted] }));
    await fireEvent.click(screen.getByRole('button', { name: /see it where it was made, round 1, lines 7–8/ }));
    expect(onround).toHaveBeenCalledWith(1, null);
  });

  it('a comment written on a file’s lines goes to the page with the file', async () => {
    const { container, oncomment } = await diffs();
    await vi.waitFor(() => expect(card(container, 'src/a.rs').querySelector('[aria-label="Line 2"]')).not.toBeNull());
    const two = [...card(container, 'src/a.rs').querySelectorAll('.diff-new [aria-label="Line 2"]')][0];
    await fireEvent.mouseDown(two, { button: 0 });
    await fireEvent.mouseUp(window);
    await fireEvent.input(screen.getByRole('textbox', { name: 'Comment on line 2' }), { target: { value: 'Why?' } });
    await fireEvent.click(within(card(container, 'src/a.rs')).getByRole('button', { name: 'Comment' }));
    expect(oncomment).toHaveBeenCalledWith({
      path: 'src/a.rs',
      picked: { side: 'new', lines: [2, 2], quote: 'TWO' },
      body: 'Why?',
    });
  });

  it('keys: ] and [ go to the next and previous file, none past either end; v marks the file at the top viewed', async () => {
    const { onfile, onviewed, container } = await diffs();
    const current = () => container.querySelector('.review-file.current [data-file]')?.getAttribute('data-file');
    await fireEvent.keyDown(window, { key: '[' });
    expect(current()).toBeUndefined();
    await fireEvent.keyDown(window, { key: ']' });
    expect(current()).toBe('README.md');
    await fireEvent.keyDown(window, { key: ']' });
    await fireEvent.keyDown(window, { key: ']' });
    expect(current()).toBe('src/a.rs');
    // Two jumps before the page is idle: the address is written once, with the last
    await vi.waitFor(() => expect(onfile).toHaveBeenCalled());
    expect(onfile.mock.calls).toEqual([['src/a.rs']]);
    await fireEvent.keyDown(window, { key: 'v' });
    expect(onviewed).toHaveBeenCalledWith(expect.objectContaining({ path: 'src/a.rs' }));
    await fireEvent.keyDown(window, { key: '[' });
    expect(current()).toBe('README.md');
    await vi.waitFor(() => expect(onfile.mock.calls.at(-1)).toEqual(['README.md']));
  });

  it('keys are the Diffs tab’s: not on another tab, not while typing', async () => {
    const { onfile } = show();
    await fireEvent.keyDown(window, { key: ']' });
    expect(onfile).not.toHaveBeenCalled();
    await fireEvent.click(tab(/^Diffs/));
    await fireEvent.keyDown(screen.getByLabelText('Round'), { key: ']' });
    expect(onfile).not.toHaveBeenCalled();
  });

  // ATM-588: Wrap, in Unified; in Split it waits, saying why (its sides kept level line by line)
  it('Wrap: in Unified it wraps every file’s lines, kept for the next page; in Split it is disabled, saying why', async () => {
    localStorage.removeItem('atm-review-layout');
    const { container, unmount } = await diffs(data({}, { files: [file('src/a.rs')] }));
    await vi.waitFor(() => expect(container.querySelector('.diff-view')).not.toBeNull());
    const wrap = () => screen.getByRole('button', { name: 'Wrap' });
    await fireEvent.click(screen.getByRole('button', { name: 'Split' }));
    expect(wrap()).toBeDisabled();
    expect(wrap()).toHaveAttribute('title', expect.stringMatching(/Unified for now/));
    expect(wrap().closest('[title]')).toHaveAttribute('title', expect.stringMatching(/level line by line/));
    await fireEvent.click(screen.getByRole('button', { name: 'Unified' }));
    expect(wrap()).toBeEnabled();
    expect(wrap()).toHaveAttribute('aria-pressed', 'false');
    await fireEvent.click(wrap());
    expect(wrap()).toHaveAttribute('aria-pressed', 'true');
    await vi.waitFor(() => expect(container.querySelector('.diff-code.wrap')).not.toBeNull());
    // Back to Split: not wrapped there, and pressed again only in Unified
    await fireEvent.click(screen.getByRole('button', { name: 'Split' }));
    await vi.waitFor(() => expect(container.querySelector('.diff-code.wrap')).toBeNull());
    expect(wrap()).toHaveAttribute('aria-pressed', 'false');
    await fireEvent.click(screen.getByRole('button', { name: 'Unified' }));
    unmount();
    await diffs(data({}, { files: [file('src/a.rs')] }));
    expect(screen.getByRole('button', { name: 'Wrap' })).toHaveAttribute('aria-pressed', 'true');
  });

  it('Split or Unified for every file, kept for the next review page; Source or Rendered only with a Markdown file', async () => {
    const { container, unmount } = await diffs(data({}, { files: [file('src/b.rs'), file('src/a.rs')] }));
    await vi.waitFor(() => expect(container.querySelectorAll('.diff-view')).toHaveLength(2));
    expect(screen.queryByRole('button', { name: 'Rendered' })).toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: 'Unified' }));
    expect([...container.querySelectorAll('.diff-view')].map((v) => v.getAttribute('data-mode'))).toEqual([
      'unified',
      'unified',
    ]);
    unmount();
    const again = await diffs(data({}, { files: [file('docs/plan.md')] }));
    await vi.waitFor(() => expect(again.container.querySelector('.diff-view')).toHaveAttribute('data-mode', 'unified'));
    await fireEvent.click(screen.getByRole('button', { name: 'Rendered' }));
    expect(screen.getByRole('button', { name: 'Rendered' })).toHaveAttribute('aria-pressed', 'true');
    await vi.waitFor(() =>
      expect(again.container.querySelector('.diff-view')).toHaveAttribute('data-view', 'rendered'),
    );
  });

  it('the splitter moves with the arrow keys, kept between its least and half the page, and the width is kept', async () => {
    const { unmount } = await diffs();
    const bar = screen.getByRole('separator', { name: 'Resize the files' });
    expect(bar).toHaveAttribute('aria-valuenow', '280');
    await fireEvent.keyDown(bar, { key: 'ArrowLeft' });
    expect(bar).toHaveAttribute('aria-valuenow', '264');
    for (let i = 0; i < 20; i++) await fireEvent.keyDown(bar, { key: 'ArrowLeft' });
    expect(bar).toHaveAttribute('aria-valuenow', '160');
    await fireEvent.keyDown(bar, { key: 'ArrowRight' });
    await fireEvent.keyDown(bar, { key: 'Enter' });
    expect(bar).toHaveAttribute('aria-valuenow', '176');
    unmount();
    await diffs();
    expect(screen.getByRole('separator')).toHaveAttribute('aria-valuenow', '176');
  });
});

describe('ReviewPage: Comments', () => {
  const chains = [
    chain('t1', 'src/a.rs'),
    chain('t2', null, {}, true),
    chain('t3', 'src/b.rs', { drifted: { round: 1, lines: [4, 5] }, placed: null }, false),
  ];

  it('every chain, open or resolved, each counted; an empty filter says so', async () => {
    const { container } = show(data({ chains }));
    await fireEvent.click(tab(/^Comments/));
    const listed = () =>
      [...container.querySelectorAll('.review-comments .chain')].map((c) => c.getAttribute('data-chain'));
    expect(
      [...container.querySelectorAll('.review-filter button')].map((b) => b.textContent?.replace(/\s+/g, ' ').trim()),
    ).toEqual(['All 3', 'Open 2', 'Resolved 1']);
    expect(listed()).toEqual(['t1', 't2', 't3']);
    await fireEvent.click(screen.getByRole('button', { name: /^Open/ }));
    expect(listed()).toEqual(['t1', 't3']);
    await fireEvent.click(screen.getByRole('button', { name: /^Resolved/ }));
    expect(listed()).toEqual(['t2']);
    // A drifted chain shows where it was made
    await fireEvent.click(screen.getByRole('button', { name: /^All/ }));
    expect(container.querySelector('[data-chain="t3"] .chain-where')?.textContent).toContain('src/b.rs · Lines 4–5');
  });

  it('nothing yet, nothing open, nothing resolved', async () => {
    show(data({ chains: [] }));
    await fireEvent.click(tab(/^Comments/));
    expect(screen.getByText('No comments yet.')).toBeTruthy();
    show(data({ chains: [chain('t2', null, {}, true)] }));
    await fireEvent.click(screen.getAllByRole('tab', { name: /^Comments/ })[1]);
    await fireEvent.click(screen.getAllByRole('button', { name: /^Open/ })[0]);
    expect(screen.getByText('Nothing open.')).toBeTruthy();
  });

  it('a chain’s place goes to it: its file on the Diffs tab, the review’s to the Overview', async () => {
    const { onfile } = show(data({ chains }));
    await fireEvent.click(tab(/^Comments/));
    await fireEvent.click(screen.getByRole('button', { name: 'src/a.rs · Line 2' }));
    expect(tab(/^Diffs/)).toHaveAttribute('aria-selected', 'true');
    await vi.waitFor(() => expect(onfile).toHaveBeenCalledWith('src/a.rs'));
    await fireEvent.click(tab(/^Comments/));
    await fireEvent.click(screen.getByRole('button', { name: 'The review' }));
    expect(tab('Overview')).toHaveAttribute('aria-selected', 'true');
  });

  it('resolving, reopening and replying are the page’s', async () => {
    const { container, onresolve, onreply } = show(data({ chains }));
    await fireEvent.click(tab(/^Comments/));
    const t1 = container.querySelector('[data-chain="t1"]') as HTMLElement;
    await fireEvent.click(within(t1).getByRole('button', { name: /Resolve/ }));
    expect(onresolve).toHaveBeenCalledWith('c-t1', true);
    const t2 = container.querySelector('[data-chain="t2"]') as HTMLElement;
    await fireEvent.click(within(t2).getByRole('button', { name: 'Show' }));
    await fireEvent.click(within(t2).getByRole('button', { name: 'Reopen' }));
    expect(onresolve).toHaveBeenCalledWith('c-t2', false);
    await fireEvent.click(within(t1).getByRole('button', { name: 'Reply' }));
    await fireEvent.input(within(t1).getByRole('textbox'), { target: { value: 'Done.' } });
    await fireEvent.click(within(t1.querySelector('form') as HTMLElement).getByRole('button', { name: 'Reply' }));
    expect(onreply).toHaveBeenCalledWith('c-t1', 'Done.');
  });
});

describe('ReviewPage: submit', () => {
  /** A review with a comment still open: the decision is Submit review's */
  const open = (more: Partial<ReviewPageData> = {}) => data({ chains: [chain('t1', 'src/a.rs')], ...more });

  it('Submit review opens the decision: request changes, with or without a summary; comment only with one', async () => {
    const { onsubmit } = show(open());
    expect(screen.queryByRole('button', { name: 'Approve' })).toBeNull();
    const toggle = screen.getByRole('button', { name: 'Submit review' });
    await fireEvent.click(toggle);
    expect(toggle).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByRole('button', { name: 'Comment' })).toBeDisabled();
    await fireEvent.click(screen.getByRole('button', { name: 'Request changes' }));
    // A decision made closes it
    expect(screen.queryByRole('button', { name: 'Request changes' })).toBeNull();
    await fireEvent.click(toggle);
    await fireEvent.input(screen.getByLabelText('Summary'), { target: { value: ' Two things. ' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Request changes' }));
    await fireEvent.click(toggle);
    await fireEvent.input(screen.getByLabelText('Summary'), { target: { value: ' Two things. ' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Comment' }));
    expect(onsubmit.mock.calls).toEqual([
      ['request_changes', ''],
      ['request_changes', 'Two things.'],
      ['comment', 'Two things.'],
    ]);
  });

  // ATM-601: pressed, it failed with atm's refusal
  it.each([
    ['a comment is open', (p: ReviewPageData) => p, '1 comment is still open: resolve them first'],
    [
      'two are open',
      (p: ReviewPageData) => ({ ...p, chains: [...p.chains, { ...p.chains[0], id: 't2' }] }),
      '2 comments are still open: resolve them first',
    ],
  ])('while %s, Approve is disabled and says why', async (_, made, why) => {
    const { onsubmit } = show(made(open()));
    // No one-click Approve either
    await fireEvent.click(screen.getByRole('button', { name: 'Submit review' }));
    const approve = screen.getByRole('button', { name: 'Approve' });
    expect(approve).toBeDisabled();
    expect(approve).toHaveAccessibleDescription(why);
    expect(approve.closest('[title]')).toHaveAttribute('title', why);
    expect(screen.getByText(why)).toBeTruthy();
    await fireEvent.click(approve);
    expect(onsubmit).not.toHaveBeenCalled();
  });

  it('nothing standing in the way, Approve in the panel is enabled, with no reason shown', async () => {
    show(data({ chains: [chain('t1', 'src/a.rs', {}, true)] }));
    await fireEvent.click(screen.getByRole('button', { name: 'Other decisions' }));
    const approve = screen.getAllByRole('button', { name: 'Approve' }).at(-1)!;
    expect(approve).toBeEnabled();
    expect(approve).not.toHaveAttribute('aria-describedby');
    expect(screen.queryByText(/still open|Sent back/)).toBeNull();
  });

  // ATM-609: a reviewer may approve a review they sent back, its comments resolved; with one open, still not
  it('sent back, everything resolved: Approve is one click, and sends the approval', async () => {
    const { onsubmit } = show(
      data({ chains: [chain('t1', 'src/a.rs', {}, true)], review: { ...open().review, status: 'changes_requested' } }),
    );
    const approve = screen.getByRole('button', { name: 'Approve' });
    expect(approve).toBeEnabled();
    expect(approve).not.toHaveAttribute('aria-describedby');
    await fireEvent.click(approve);
    expect(onsubmit.mock.calls).toEqual([['approve', '']]);
  });

  it('sent back with a comment open: Approve is disabled, saying so', async () => {
    show({ ...open(), review: { ...open().review, status: 'changes_requested' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Submit review' }));
    expect(screen.getByRole('button', { name: 'Approve' })).toHaveAccessibleDescription(
      '1 comment is still open: resolve them first',
    );
  });

  it('a refresh keeps the summary being written; a decision made (the review updated) empties it', async () => {
    const { rerender } = show(open());
    await fireEvent.click(screen.getByRole('button', { name: 'Submit review' }));
    await fireEvent.input(screen.getByLabelText('Summary'), { target: { value: 'Half written' } });
    // The same review, asked for again: a new object, nothing changed
    await rerender({ page: open() });
    expect(screen.getByLabelText('Summary')).toHaveValue('Half written');
    const decided = open();
    decided.review = { ...decided.review, updated: '2026-11-17T11:00:00Z' };
    await rerender({ page: decided });
    expect(screen.getByLabelText('Summary')).toHaveValue('');
  });

  it('a review sent back still takes the reviewer’s words, as Publish comments; it isn’t sent back again (ATM-597, CR-2)', async () => {
    const page = open();
    page.review = { ...page.review, status: 'changes_requested' };
    const { onsubmit } = show(page);
    expect(screen.getByRole('button', { name: 'Comment on the review' })).toBeEnabled();
    await fireEvent.click(screen.getByRole('button', { name: 'Submit review' }));
    expect(screen.queryByRole('button', { name: 'Request changes' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Comment' })).toBeNull();
    const publish = screen.getByRole('button', { name: 'Publish comments' });
    expect(publish).toBeDisabled();
    await fireEvent.input(screen.getByLabelText('Summary'), { target: { value: 'Still the title.' } });
    await fireEvent.click(publish);
    expect(onsubmit.mock.calls).toEqual([['comment', 'Still the title.']]);
  });

  it('a live review (ATM-576): approved, commented on or closed with why, never sent back', async () => {
    const page = open();
    page.review = { ...page.review, status: 'live' };
    const onclose = vi.fn();
    const { onsubmit } = show(page, { onclose });
    await fireEvent.click(screen.getByRole('button', { name: 'Submit review' }));
    expect(screen.queryByRole('button', { name: 'Request changes' })).toBeNull();
    const close = screen.getByRole('button', { name: 'Close review' });
    expect(close).toBeDisabled();
    await fireEvent.input(screen.getByLabelText('Summary'), { target: { value: ' Wrong turn. ' } });
    await fireEvent.click(close);
    expect(onclose).toHaveBeenCalledWith('Wrong turn.');
    expect(onsubmit).not.toHaveBeenCalled();
  });

  it('an ended review takes no decision; a busy page none until it is done', async () => {
    for (const page of [open(), data()]) {
      for (const status of ['approved', 'closed'] as const) {
        page.review = { ...page.review, status };
        const { unmount } = show(page);
        expect(screen.queryByRole('button', { name: /Submit review|Approve/ })).toBeNull();
        unmount();
      }
    }
    show(open(), { busy: true });
    await fireEvent.click(screen.getByRole('button', { name: 'Submit review' }));
    expect(screen.getByRole('button', { name: 'Approve' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Request changes' })).toBeDisabled();
  });

  it('every comment resolved (or none made): Submit review is Approve, one click; the other decisions beside it', async () => {
    const { onsubmit, unmount } = show(data({ chains: [chain('t1', 'src/a.rs', {}, true)] }));
    expect(screen.queryByRole('button', { name: 'Submit review' })).toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: 'Approve' }));
    expect(onsubmit).toHaveBeenCalledWith('approve', '');
    const more = screen.getByRole('button', { name: 'Other decisions' });
    await fireEvent.click(more);
    expect(more).toHaveAttribute('aria-expanded', 'true');
    await fireEvent.input(screen.getByLabelText('Summary'), { target: { value: 'One more thing.' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Request changes' }));
    expect(onsubmit).toHaveBeenLastCalledWith('request_changes', 'One more thing.');
    unmount();
    // No comments at all: nothing left open either
    show(data({ chains: [] }), { busy: true });
    expect(screen.getByRole('button', { name: 'Approve' })).toBeDisabled();
  });

  it('a comment on the review the page couldn’t save stays, with why', async () => {
    const oncomment = vi.fn().mockResolvedValueOnce('the review is closed').mockResolvedValueOnce(null);
    show(data(), { oncomment });
    await fireEvent.click(screen.getByRole('button', { name: 'Comment on the review' }));
    await fireEvent.input(screen.getByLabelText('Comment'), { target: { value: 'Keep me.' } });
    const save = within(document.querySelector('.review-composer') as HTMLElement).getByRole('button', {
      name: 'Comment',
    });
    await fireEvent.click(save);
    expect(await screen.findByText('Not saved: the review is closed')).toBeTruthy();
    expect(screen.getByLabelText('Comment')).toHaveValue('Keep me.');
    await fireEvent.click(save);
    await vi.waitFor(() => expect(screen.queryByLabelText('Comment')).toBeNull());
  });
});
