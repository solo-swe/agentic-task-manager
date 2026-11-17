import { fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DocumentResult } from '../../src/lib/documents';
import type { DocumentSource } from '../../src/lib/documents/source';
import ReviewFile, { MARGIN, holdNearCards, inView, whenIdle } from '../../src/lib/reviews/ReviewFile.svelte';
import type { PlacedChain, ViewFile } from '../../src/lib/reviews/types';

// A file's card on a review's Diffs tab (lib/reviews/ReviewFile.svelte, ATM-574): its header, its fold, and its diff,
// made once the card nears the view. The diff viewer is its own unit (DiffView.component.test.ts); here its sides are
// stubs and CompMD doesn't load.
const loadCompmd = vi.hoisted(() => vi.fn());
vi.mock('../../src/lib/compmd', async (actual) => ({ ...(await actual()), loadCompmd }));

const file = (more: Partial<ViewFile> = {}): ViewFile => ({
  path: 'src/a.rs',
  old_path: 'src/a.rs',
  status: 'modified',
  binary: false,
  patch: '@@ -1,2 +1,3 @@\n one\n-two\n+TWO\n+three\n',
  before: 'b',
  after: 'a',
  ...more,
});

const side = (text: string): DocumentSource => ({
  id: text,
  load: async (path): Promise<DocumentResult> => ({
    unchanged: false,
    document: {
      path,
      kind: 'code',
      language: 'text',
      text,
      document: text,
      line_offset: 1,
      lines: text.split('\n').length - 1,
      bytes: text.length,
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

function show(more: Record<string, unknown> = {}) {
  const handlers = {
    onfold: vi.fn(),
    onviewed: vi.fn(),
    oncomment: vi.fn(),
    onreply: vi.fn(),
    onresolve: vi.fn(),
    onround: vi.fn(),
  };
  const result = render(ReviewFile, {
    file: file(),
    chains: [],
    drifted: [],
    before: side('one\ntwo\n'),
    after: side('one\nTWO\nthree\n'),
    viewed: false,
    folded: false,
    mode: 'split',
    view: 'source',
    ...handlers,
    ...more,
  });
  return { ...result, ...handlers };
}

/** An IntersectionObserver the test drives: `seen()` brings every card it watches into view */
function observer() {
  const watched: { callback: IntersectionObserverCallback; options?: IntersectionObserverInit }[] = [];
  const disconnect = vi.fn();
  class Fake {
    constructor(callback: IntersectionObserverCallback, options?: IntersectionObserverInit) {
      watched.push({ callback, options });
    }
    observe() {}
    disconnect = disconnect;
  }
  vi.stubGlobal('IntersectionObserver', Fake);
  return {
    watched,
    disconnect,
    seen: (isIntersecting = true) =>
      watched.forEach((w) => w.callback([{ isIntersecting } as IntersectionObserverEntry], {} as never)),
  };
}

/** A box from `top` to `bottom` */
const rect = (top: number, bottom: number) => ({ top, bottom, left: 0, right: 100, width: 100, height: bottom - top });

describe('when a card is made', () => {
  // The observer's box is the list's grown by MARGIN on each side: the view is what is inside the margin
  const entry = (top: number, bottom: number, bounds: ReturnType<typeof rect> | null = rect(0, 600 + 2 * MARGIN)) =>
    ({ rootBounds: bounds, boundingClientRect: rect(top, bottom) }) as unknown as IntersectionObserverEntry;

  it.each([
    ['inside the view', MARGIN + 10, MARGIN + 100, true],
    ['across its top edge', MARGIN - 50, MARGIN + 50, true],
    ['across its bottom edge', MARGIN + 550, MARGIN + 650, true],
    ['in the margin above', 10, MARGIN - 10, false],
    ['in the margin below', MARGIN + 610, MARGIN + 700, false],
    ['ending at the view’s top', MARGIN - 100, MARGIN, false],
  ])('a card %s: in view %s', (_, top, bottom, expected) => {
    expect(inView(entry(top, bottom))).toBe(expected);
  });

  it('without the root’s box (no layout), a card counts as in view', () => {
    expect(inView(entry(5000, 5100, null))).toBe(true);
  });

  it('later, when the page is idle; or after a moment where there is no idle callback; either can be cancelled', () => {
    const request = vi.fn((act: () => void, options: { timeout: number }) => (void [act, options], 3));
    const cancel = vi.fn();
    vi.stubGlobal('requestIdleCallback', request);
    vi.stubGlobal('cancelIdleCallback', cancel);
    const act = vi.fn();
    whenIdle(act)();
    expect(request).toHaveBeenCalledWith(act, { timeout: 1000 });
    expect(cancel).toHaveBeenCalledWith(3);
    vi.unstubAllGlobals();
    vi.stubGlobal('requestIdleCallback', undefined);
    vi.useFakeTimers();
    try {
      whenIdle(act);
      vi.advanceTimersByTime(149);
      expect(act).not.toHaveBeenCalled();
      vi.advanceTimersByTime(1);
      expect(act).toHaveBeenCalledOnce();
      whenIdle(act)();
      vi.advanceTimersByTime(500);
      expect(act).toHaveBeenCalledOnce();
    } finally {
      vi.useRealTimers();
    }
  });
});

beforeEach(() => {
  vi.spyOn(console, 'error').mockImplementation(() => {});
  loadCompmd.mockRejectedValue(new Error('no CompMD in these tests'));
});
afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe('ReviewFile: the header', () => {
  it('its status, path, what it adds and removes, its comments and Viewed', async () => {
    const chain = (id: string) =>
      ({ id, anchor: { path: 'src/a.rs', side: 'new', lines: null }, comments: [] }) as unknown as PlacedChain;
    const { container, onviewed } = show({ chains: [chain('t1'), chain('t2')] });
    const head = container.querySelector('.review-file-head') as HTMLElement;
    expect(head.textContent?.replace(/\s+/g, ' ').trim()).toBe('M src/a.rs +2 −1 2 comments Viewed');
    expect(head.querySelector('.review-fold .icon-chevron-down')).not.toBeNull();
    expect(container.querySelector('.review-file-card')).toHaveAttribute('data-comments');
    await fireEvent.click(screen.getByLabelText('Viewed src/a.rs'));
    expect(onviewed).toHaveBeenCalledOnce();
  });

  it.each([
    [{ status: 'added' as const }, 'A', 'src/a.rs'],
    [{ status: 'deleted' as const }, 'D', 'src/a.rs'],
    [{ status: 'renamed' as const, old_path: 'src/old.rs' }, 'R', 'src/old.rs → src/a.rs'],
  ])('%o: %s %s', (more, letter, path) => {
    const { container } = show({ file: file(more) });
    expect(container.querySelector('.review-file-status')?.textContent).toBe(letter);
    expect(container.querySelector('.review-file-path')?.textContent).toBe(path);
  });

  it('one comment is one; a binary file says so instead of counts', () => {
    const chain = { id: 't', anchor: { path: 'src/a.rs' }, comments: [] } as unknown as PlacedChain;
    const { container } = show({ chains: [chain], file: file({ binary: true, patch: '' }) });
    expect(container.querySelector('.review-file-chains')?.textContent).toBe('1 comment');
    expect(container.querySelector('.review-file-counts')?.textContent?.trim()).toBe('binary');
    expect(screen.getByText('A binary file: listed, not shown.')).toBeTruthy();
    expect(container.querySelector('.diff-view')).toBeNull();
  });
});

describe('ReviewFile: a submodule’s move (ATM-583)', () => {
  const moved = (from: string | null, to: string | null, repository: string | null = null) =>
    file({
      path: 'lib',
      old_path: 'lib',
      status: from && to ? 'modified' : to ? 'added' : 'deleted',
      patch: '@@ -1 +1 @@\n-Subproject commit x\n+Subproject commit y\n',
      submodule: { from, to, repository },
    });

  it.each([
    ['moved', moved('1111111aaaa', '2222222bbbb'), 'The submodule moved from 1111111 to 2222222.'],
    ['added', moved(null, '3333333cccc'), 'The submodule was added, at 3333333.'],
    ['removed', moved('4444444dddd', null), 'The submodule was removed; it was at 4444444.'],
  ])('%s: says so with its commits, no diff and no lines loaded', (_, entry, says) => {
    const load = vi.fn();
    const { container } = show({ file: entry, before: { ...side(''), load }, after: { ...side(''), load } });
    expect(container.querySelector('.review-submodule-move')?.textContent?.replace(/\s+/g, ' ').trim()).toBe(says);
    expect(container.querySelector('.review-file-counts')?.textContent?.trim()).toBe('submodule');
    expect(container.querySelector('.diff-view')).toBeNull();
    expect(container.querySelector('.review-file-card')).not.toHaveAttribute('data-pending');
    expect(load).not.toHaveBeenCalled();
    expect(container.querySelector('.review-submodule-move a')).toBeNull();
  });

  it('its commits link to its repository when it has a web address', () => {
    const { container } = show({ file: moved('1111111aaaa', '2222222bbbb', 'https://github.com/o/lib') });
    const links = [...container.querySelectorAll('.review-submodule-move a')].map((a) => a.getAttribute('href'));
    expect(links).toEqual([
      'https://github.com/o/lib/commit/1111111aaaa',
      'https://github.com/o/lib/commit/2222222bbbb',
    ]);
  });

  it('a comment on it is on the entry as a whole; its comments show under it', async () => {
    const chain = {
      id: 't1',
      anchor: { round: 1, path: 'lib', side: 'new', lines: null, quote: null },
      comments: [
        {
          id: 'c1',
          author: 'Hakan Serce',
          at: '2026-11-18T10:00:00Z',
          body: 'Why this version?',
          resolved: false,
          replies: [],
        },
      ],
    } as unknown as PlacedChain;
    const { container, oncomment } = show({ file: moved('1111111aaaa', '2222222bbbb'), chains: [chain] });
    // The chain, as a whole file's (its body is CompMD's to render, not loaded here)
    expect(container.querySelector('.review-submodule')?.textContent).toMatch(/The file[\s\S]*Hakan Serce/);
    await fireEvent.click(screen.getByText('Comment on this submodule'));
    const box = screen.getByLabelText('Comment on lib') as HTMLTextAreaElement;
    expect(screen.getByText('Comment')).toBeDisabled();
    await fireEvent.input(box, { target: { value: '  Pin it.  ' } });
    await fireEvent.click(screen.getByText('Comment'));
    expect(oncomment).toHaveBeenCalledWith(null, 'Pin it.');
    expect(screen.queryByLabelText('Comment on lib')).toBeNull();
  });

  it('a comment refused stays, saying why; Cancel and Escape close the box', async () => {
    const oncomment = vi.fn(async () => 'the review is closed');
    show({ file: moved('1111111aaaa', '2222222bbbb'), oncomment });
    await fireEvent.click(screen.getByText('Comment on this submodule'));
    const box = screen.getByLabelText('Comment on lib') as HTMLTextAreaElement;
    await fireEvent.input(box, { target: { value: 'Pin it.' } });
    await fireEvent.keyDown(box, { key: 'Enter', metaKey: true });
    expect(await screen.findByRole('alert')).toHaveTextContent('Not saved: the review is closed');
    expect(box.value).toBe('Pin it.');
    await fireEvent.keyDown(box, { key: 'Escape' });
    expect(screen.queryByLabelText('Comment on lib')).toBeNull();
    await fireEvent.click(screen.getByText('Comment on this submodule'));
    await fireEvent.click(screen.getByText('Cancel'));
    expect(screen.queryByLabelText('Comment on lib')).toBeNull();
  });
});

describe('ReviewFile: folding', () => {
  it('folded: the header alone, the fold says Show; open: the diff, the fold says Hide', async () => {
    const { container, onfold, rerender } = show({ folded: true, viewed: true });
    expect(container.querySelector('.review-file-body')).toBeNull();
    expect(container.querySelector('.review-fold .icon-chevron-right')).not.toBeNull();
    expect(container.querySelector('.review-file-card')).not.toHaveAttribute('data-comments');
    expect(container.querySelector('.review-file-viewed .icon-check')).not.toBeNull();
    const fold = screen.getByRole('button', { name: 'Show src/a.rs' });
    expect(fold).toHaveAttribute('aria-expanded', 'false');
    await fireEvent.click(fold);
    expect(onfold).toHaveBeenCalledOnce();
    await rerender({ folded: false });
    expect(screen.getByRole('button', { name: 'Hide src/a.rs' })).toHaveAttribute('aria-expanded', 'true');
    await vi.waitFor(() => expect(container.querySelector('.diff-view')).not.toBeNull());
  });

  it('a drifted chain links to the round it was made on', async () => {
    const drifted = { id: 'd', drifted: { round: 2, lines: null } } as unknown as PlacedChain;
    const { onround } = show({ drifted: [drifted] });
    await fireEvent.click(screen.getByRole('button', { name: 'see it where it was made, round 2' }));
    expect(onround).toHaveBeenCalledWith(2);
  });
});

describe('ReviewFile: its diff, made when near the view', () => {
  it('waits, as tall as about its lines, until the card nears the list’s view; then made, and kept', async () => {
    const io = observer();
    const root = document.createElement('div');
    const { container } = show({ root });
    expect(container.querySelector('.diff-view')).toBeNull();
    expect(container.querySelector('.review-file-later')).toHaveStyle({ height: '220px' });
    // The page's Next goes to it to make it
    expect(container.querySelector('.review-file-card')).toHaveAttribute('data-pending');
    expect(io.watched[0].options).toMatchObject({ root, rootMargin: '800px 0px' });
    io.seen(false);
    expect(container.querySelector('.diff-view')).toBeNull();
    io.seen();
    await vi.waitFor(() => expect(container.querySelector('.diff-view')).not.toBeNull());
    expect(container.querySelector('.review-file-card')).not.toHaveAttribute('data-pending');
    expect(io.disconnect).toHaveBeenCalled();
  });

  it('a card only near the view waits for the page to be idle; one that comes into view meanwhile is made at once', async () => {
    const io = observer();
    let idle: (() => void) | null = null;
    vi.stubGlobal('requestIdleCallback', (act: () => void) => ((idle = act), 7));
    const cancelIdleCallback = vi.fn();
    vi.stubGlobal('cancelIdleCallback', cancelIdleCallback);
    const { container } = show();
    const near = { isIntersecting: true, rootBounds: rect(0, 2600), boundingClientRect: rect(1900, 2000) };
    io.watched[0].callback([near as unknown as IntersectionObserverEntry], {} as never);
    await Promise.resolve();
    expect(container.querySelector('.diff-view')).toBeNull();
    idle!();
    await vi.waitFor(() => expect(container.querySelector('.diff-view')).not.toBeNull());
    // Another card: near, then in view before the page is idle
    const other = show();
    const view = { ...near, boundingClientRect: rect(900, 1000) };
    io.watched[1].callback([near as unknown as IntersectionObserverEntry], {} as never);
    io.watched[1].callback([view as unknown as IntersectionObserverEntry], {} as never);
    expect(cancelIdleCallback).toHaveBeenCalledWith(7);
    await vi.waitFor(() => expect(other.container.querySelector('.diff-view')).not.toBeNull());
  });

  it('right after a jump, a card in view waits too; the file gone to (wanted) is made at once', async () => {
    const io = observer();
    let idle: (() => void) | null = null;
    vi.stubGlobal('requestIdleCallback', (act: () => void) => ((idle = act), 1));
    vi.stubGlobal('cancelIdleCallback', vi.fn());
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'performance'] });
    try {
      holdNearCards(400);
      const { container } = show();
      const inViewEntry = { isIntersecting: true, rootBounds: rect(0, 2600), boundingClientRect: rect(900, 1000) };
      io.watched[0].callback([inViewEntry as unknown as IntersectionObserverEntry], {} as never);
      // Idle, but still held: it waits out the hold, then for the page to be idle again
      idle!();
      idle = null;
      await Promise.resolve();
      expect(container.querySelector('.diff-view')).toBeNull();
      vi.advanceTimersByTime(400);
      expect(idle).not.toBeNull();
      idle!();
      vi.useRealTimers();
      await vi.waitFor(() => expect(container.querySelector('.diff-view')).not.toBeNull());
      // The wanted one, without waiting for anything
      holdNearCards(400);
      const wanted = show({ wanted: true });
      await vi.waitFor(() => expect(wanted.container.querySelector('.diff-view')).not.toBeNull());
    } finally {
      vi.useRealTimers();
      holdNearCards(0);
    }
  });

  it('a card gone before the page is idle isn’t made', () => {
    const io = observer();
    vi.stubGlobal('requestIdleCallback', () => 9);
    const cancelIdleCallback = vi.fn();
    vi.stubGlobal('cancelIdleCallback', cancelIdleCallback);
    const { unmount } = show();
    const near = { isIntersecting: true, rootBounds: rect(0, 2600), boundingClientRect: rect(1900, 2000) };
    io.watched[0].callback([near as unknown as IntersectionObserverEntry], {} as never);
    unmount();
    expect(cancelIdleCallback).toHaveBeenCalledWith(9);
  });

  it('a folded card or a binary file isn’t waiting to be made', () => {
    observer();
    const folded = show({ folded: true });
    expect(folded.container.querySelector('.review-file-card')).not.toHaveAttribute('data-pending');
    folded.unmount();
    const binary = show({ file: file({ binary: true, patch: '' }) });
    expect(binary.container.querySelector('.review-file-card')).not.toHaveAttribute('data-pending');
  });

  it('gives its diff the page’s layout and view, no header and no keys of its own; a comment goes up with its lines', async () => {
    const { container, oncomment } = show({ mode: 'unified' });
    await vi.waitFor(() => expect(container.querySelector('.diff-view')).toHaveAttribute('data-mode', 'unified'));
    expect(container.querySelector('.diff-header')).toBeNull();
    // Unified: the old line 2 removed, then the new line 2 added
    const [, two] = await screen.findAllByRole('button', { name: 'Line 2' });
    await fireEvent.mouseDown(two, { button: 0 });
    await fireEvent.mouseUp(window);
    await fireEvent.input(screen.getByRole('textbox', { name: 'Comment on line 2' }), { target: { value: 'Hm' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Comment' }));
    expect(oncomment).toHaveBeenCalledWith(expect.objectContaining({ lines: [2, 2] }), 'Hm');
  });
});
