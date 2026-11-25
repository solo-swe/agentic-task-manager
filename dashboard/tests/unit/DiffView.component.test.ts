import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import DiffView from '../../src/lib/reviews/DiffView.svelte';
import type { DocumentResult, ProjectDocument } from '../../src/lib/documents';
import type { DocumentSource } from '../../src/lib/documents/source';
import type { Chain } from '../../src/lib/reviews/types';

// The diff viewer (docs/code-review-design.md §3.7, ATM-573, ATM-574): a round's file, its sides from document sources
// made up here (CompMD doesn't load in these tests, so each run of lines shows as written, 20 px a line, with the
// viewer's gutter beside it)
const loadCompmd = vi.hoisted(() => vi.fn());
vi.mock('../../src/lib/compmd', async (actual) => ({ ...(await actual()), loadCompmd }));

const OLD = ['one', 'two', 'three', 'four', 'five', 'six', 'seven', 'eight', 'nine', 'ten'].join('\n') + '\n';
const NEW =
  ['one', 'two', 'THREE', 'four', 'five', 'six', 'seven', 'eight a', 'eight b', 'nine', 'ten'].join('\n') + '\n';
const PATCH = [
  '@@ -2,3 +2,3 @@',
  ' two',
  '-three',
  '+THREE',
  ' four',
  '@@ -7,3 +7,4 @@',
  ' seven',
  '-eight',
  '+eight a',
  '+eight b',
  ' nine',
  '',
].join('\n');

const doc = (path: string, text: string, kind: ProjectDocument['kind'] = 'code'): ProjectDocument => ({
  path,
  kind,
  language: kind === 'code' ? 'text' : null,
  text,
  document: text,
  line_offset: 1,
  lines: text.split('\n').length - 1,
  bytes: text.length,
  modified_at: null,
  tracked: true,
  commit: 'abc1234',
  submodule: null,
});

/** One side: the round's base or head, at its revision; `loads` counts what it was asked for */
const at = (id: string, text: string | null, kind?: ProjectDocument['kind']) => {
  const load = vi.fn(async (path: string): Promise<DocumentResult> =>
    text === null
      ? { unchanged: false, error: 'missing', message: `${path}: not in ${id}` }
      : { unchanged: false, document: doc(path, text, kind), validators: {} },
  );
  return { id, load, imageUrl: () => null, repositoryLink: () => null } satisfies DocumentSource;
};

const chain = (id: string, side: 'old' | 'new', lines: [number, number], body: string, resolved = false): Chain => ({
  id,
  anchor: { round: 1, path: 'f.txt', side, lines, quote: null },
  comments: [{ id: `c-${id}`, author: 'Hakan Serce', at: '2026-11-17T10:00:00Z', body, resolved, replies: [] }],
});

function show(props: Partial<Parameters<typeof render<typeof DiffView>>[1] & object> = {}) {
  const handlers = {
    oncomment: vi.fn(),
    onreply: vi.fn(),
    onresolve: vi.fn(),
    onviewed: vi.fn(),
    onnextfile: vi.fn(),
    onprevfile: vi.fn(),
  };
  const result = render(DiffView, {
    path: 'f.txt',
    before: at('base', OLD),
    after: at('head', NEW),
    patch: PATCH,
    ...handlers,
    ...props,
  });
  return { ...result, ...handlers };
}

/** The gutter's line buttons, once the runs are laid out */
const numbers = async () => screen.findAllByRole('button', { name: /^Line \d+$/ });
/** Line `n`'s number on a side, in its first run that has it */
const lineOn = (container: HTMLElement, side: 'old' | 'new', n: number) =>
  [...container.querySelectorAll(`.diff-${side}`)]
    .flatMap((s) => within(s as HTMLElement).queryAllByRole('button', { name: `Line ${n}` }))
    .at(0)!;
/** A press on a line (its number or its +), and the mouse let go */
async function press(el: Element, shiftKey = false) {
  await fireEvent.mouseDown(el, { button: 0, shiftKey });
  await fireEvent.mouseUp(window);
}
const composer = () => screen.queryByRole('textbox', { name: /^Comment on line/ });

beforeEach(() => {
  vi.spyOn(console, 'error').mockImplementation(() => {});
  loadCompmd.mockRejectedValue(new Error('no CompMD in these tests'));
  Element.prototype.scrollIntoView = vi.fn();
});

afterEach(() => {
  vi.restoreAllMocks();
  loadCompmd.mockReset();
});

describe('DiffView: the layout', () => {
  it('split: each side’s lines, the changes marked, the unchanged runs folded until shown', async () => {
    const { container } = show();
    await numbers();
    expect(container.querySelectorAll('.diff-run')).toHaveLength(2);
    // Line 3 changed (marked on both sides), line 9 only added (a padding line on the old side)
    expect(container.querySelectorAll('.diff-line.diff-changed')).toHaveLength(4);
    expect(container.querySelectorAll('.diff-new .diff-line.diff-added')).toHaveLength(1);
    expect(container.querySelectorAll('.diff-old .diff-line.diff-pad')).toHaveLength(1);
    const folds = screen.getAllByRole('button', { name: /^Show \d+ unchanged lines?$/ });
    expect(folds.map((b) => b.textContent?.trim())).toEqual([
      'Show 1 unchanged line',
      'Show 2 unchanged lines',
      'Show 1 unchanged line',
    ]);
    await fireEvent.click(folds[1]);
    expect(screen.getAllByRole('button', { name: /^Show/ })).toHaveLength(2);
    // The two hunks and the lines between them are one run now
    await vi.waitFor(() => expect(container.querySelectorAll('.diff-run')).toHaveLength(1));
  });

  it('unified: one column, both numbers, removed and added lines in order', async () => {
    const { container } = show({ mode: 'unified' });
    await numbers();
    expect(container.querySelectorAll('.diff-run .diff-column')).toHaveLength(2);
    expect(container.querySelectorAll('.diff-line.diff-removed')).toHaveLength(2);
    expect(container.querySelectorAll('.diff-line.diff-added')).toHaveLength(3);
    expect(container.querySelector('.diff-code.both')).not.toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: 'Split' }));
    expect(container.querySelector('.diff-view')).toHaveAttribute('data-mode', 'split');
  });

  // ATM-731: the file as it is after the change
  it('final: every line of the new file, its numbers only, nothing marked or folded; the header chooses it', async () => {
    const { container } = show({ mode: 'final' });
    await numbers();
    expect(container.querySelector('.diff-view')).toHaveAttribute('data-mode', 'final');
    // The new file's eleven lines, each numbered on the new side alone (the text is CompMD's, not loaded here)
    const numbered = [...container.querySelectorAll('.diff-line')].map((l) => l.textContent?.trim());
    expect(numbered).toEqual(Array.from({ length: 11 }, (_, i) => String(i + 1)));
    for (const marked of ['.diff-removed', '.diff-added', '.diff-changed', '[data-change]'])
      expect(container.querySelectorAll(marked), marked).toHaveLength(0);
    expect(screen.queryAllByRole('button', { name: /^Show \d+ unchanged/ })).toHaveLength(0);
    expect(container.querySelector('.diff-code.both'), 'one number a line').toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: 'Unified' }));
    expect(container.querySelector('.diff-view')).toHaveAttribute('data-mode', 'unified');
    await fireEvent.click(screen.getByRole('button', { name: 'Final' }));
    expect(container.querySelector('.diff-view')).toHaveAttribute('data-mode', 'final');
  });

  it('final: a chain stays under its line; one on a line the change removed shows after the file', async () => {
    const { container } = show({
      mode: 'final',
      chains: [
        chain('t1', 'new', [9, 9], 'On eight b'),
        chain('t2', 'old', [5, 5], 'On five, unchanged'),
        chain('t3', 'old', [8, 8], 'On eight, removed'),
      ],
    });
    await numbers();
    const inRuns = [...container.querySelectorAll('.diff-run + .chain, .chain')].filter(
      (c) => !c.closest('.diff-elsewhere'),
    );
    expect(inRuns.map((c) => c.getAttribute('data-chain'))).toEqual(['t2', 't1']);
    const after = container.querySelector('.diff-elsewhere');
    expect(after?.querySelector('[data-chain="t3"]')).not.toBeNull();
  });

  it('final: a file the change deleted says so, with nothing after it', async () => {
    const { container } = show({ mode: 'final', after: null });
    await vi.waitFor(() => expect(container.querySelector('.diff-gone')).not.toBeNull());
    expect(container.querySelector('.diff-gone')).toHaveTextContent('Deleted by this change');
    expect(container.querySelectorAll('.diff-line')).toHaveLength(0);
  });

  it('marks the first line of each change for the page’s Prev and Next: the new side’s in split, every one in unified', async () => {
    const { container, unmount } = show();
    await numbers();
    expect([...container.querySelectorAll('[data-change]')].map((l) => (l as HTMLElement).dataset.row)).toEqual([
      '1-1',
      '3-1',
    ]);
    expect(container.querySelector('.diff-old [data-change]')).toBeNull();
    unmount();
    const unified = show({ mode: 'unified' });
    await numbers();
    // Unified: the removed line starts the first change, the removed one the second
    expect(unified.container.querySelectorAll('[data-change]')).toHaveLength(2);
  });

  it('its own header (path, switches, Viewed) only when asked: the review page has its own', async () => {
    const { container, unmount } = show({ oldPath: 'old.txt' });
    await numbers();
    expect(container.querySelector('.diff-header')).toHaveTextContent('old.txt → f.txt');
    expect(container.querySelector('.diff-view')).toHaveClass('framed');
    unmount();
    const bare = show({ header: false });
    await numbers();
    expect(bare.container.querySelector('.diff-header')).toBeNull();
    expect(screen.queryByRole('button', { name: 'Split' })).toBeNull();
  });

  it('a new file has no old side; a side that can’t be read says why', async () => {
    const { container, unmount } = show({
      before: null,
      patch: '@@ -0,0 +1,2 @@\n+a\n+b\n',
      after: at('head', 'a\nb\n'),
    });
    await numbers();
    expect(container.querySelectorAll('.diff-new .diff-line.diff-added')).toHaveLength(2);
    expect(container.querySelectorAll('.diff-old .diff-line.diff-pad')).toHaveLength(2);
    // Hakan: no line between two lines. The two added lines are tinted by one box, the two padding lines by another
    expect(container.querySelectorAll('.diff-new .diff-tint')).toHaveLength(1);
    expect(container.querySelector('.diff-new .diff-tint')).toHaveClass('diff-added');
    expect(container.querySelectorAll('.diff-old .diff-tint')).toHaveLength(1);
    expect(container.querySelector('.diff-old .diff-tint')).toHaveClass('diff-pad');
    unmount();
    show({ after: at('head', null) });
    expect(await screen.findByRole('alert')).toHaveTextContent('f.txt: not in head');
  });

  it('a long file is drawn 100 rows at a time, each chunk when the page is idle, standing at the rest’s height meanwhile (ATM-587)', async () => {
    const waiting = new Map<number, () => void>();
    let next = 0;
    vi.stubGlobal('requestIdleCallback', (act: () => void) => (waiting.set(++next, act), next));
    vi.stubGlobal('cancelIdleCallback', (id: number) => waiting.delete(id));
    const idle = {
      shift: () => {
        const [id, act] = [...waiting][0];
        waiting.delete(id);
        return act;
      },
      get length() {
        return waiting.size;
      },
    };
    try {
      const lines = Array.from({ length: 250 }, (_, i) => `line ${i + 1}`);
      const { container } = show({
        before: null,
        patch: `@@ -0,0 +1,250 @@\n${lines.map((l) => `+${l}`).join('\n')}\n`,
        after: at('head', lines.join('\n') + '\n'),
      });
      await numbers();
      const drawn = () => container.querySelectorAll('.diff-new .diff-line.diff-added').length;
      expect(drawn()).toBe(100);
      expect(container.querySelector('.diff-more')).toHaveStyle({ height: `${150 * 20}px` });
      idle.shift()!();
      await vi.waitFor(() => expect(drawn()).toBe(200));
      idle.shift()!();
      await vi.waitFor(() => expect(drawn()).toBe(250));
      expect(container.querySelector('.diff-more')).toBeNull();
      expect(idle).toHaveLength(0);
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it('the same sides given again (a refresh) keep what is shown: no Loading, nothing asked again', async () => {
    const before = at('base', OLD);
    const after = at('head', NEW);
    const { rerender, container } = show({ before, after });
    await numbers();
    await rerender({ chains: [chain('t1', 'new', [3, 3], 'Why upper case?')] });
    expect(screen.queryByText('Loading…')).toBeNull();
    expect(container.querySelector('[data-chain="t1"]')).not.toBeNull();
    expect(screen.getAllByRole('button', { name: /^Line \d+$/ }).length).toBeGreaterThan(0);
    // Another revision of the same file (a live review's working copy, a new tree at each change, ATM-687): what is
    // shown stays while it loads, never Loading
    await rerender({ after: at('head2', NEW) });
    expect(screen.queryByText('Loading…')).toBeNull();
    expect(screen.getAllByRole('button', { name: /^Line \d+$/ }).length).toBeGreaterThan(0);
    await numbers();
    // Another file: Loading until its sides are there
    await rerender({ path: 'g.txt', oldPath: 'g.txt', after: at('head3', NEW) });
    expect(screen.getByText('Loading…')).toBeInTheDocument();
    await numbers();
  });
});

describe('DiffView: picking lines and commenting', () => {
  it('the line under the pointer shows its +; pressing it picks the line and opens the comment under it', async () => {
    const { container, oncomment } = show();
    await numbers();
    const newRun = container.querySelectorAll('.diff-new .diff-code')[1] as HTMLElement;
    // Line 9 is the run's third (20 px a line)
    await fireEvent.mouseMove(newRun, { clientY: 45 });
    expect(within(newRun).getAllByRole('button', { name: /^Comment on line/ })).toHaveLength(1);
    await fireEvent.mouseLeave(newRun);
    expect(within(newRun).queryByRole('button', { name: /^Comment on line/ })).toBeNull();
    await fireEvent.mouseMove(newRun, { clientY: 45 });
    await press(within(newRun).getByRole('button', { name: 'Comment on line 9' }));
    expect(screen.getByText('Line 9')).toBeInTheDocument();
    // While the comment is written, the + stays on the line picked
    expect(within(newRun).getByRole('button', { name: 'Comment on line 9' })).toHaveClass('held');
    await fireEvent.input(composer()!, { target: { value: 'Split this?' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Comment' }));
    expect(oncomment).toHaveBeenCalledWith({ side: 'new', lines: [9, 9], quote: 'eight b' }, 'Split this?');
    expect(composer()).toBeNull();
  });

  it('dragging from a line picks every line to the one under the pointer, either way; the comment opens on letting go', async () => {
    const { container, oncomment } = show();
    await numbers();
    const newRun = container.querySelectorAll('.diff-new .diff-code')[1] as HTMLElement;
    await fireEvent.mouseDown(lineOn(container, 'new', 7), { button: 0 });
    await fireEvent.mouseMove(newRun, { clientY: 45 });
    expect(container.querySelectorAll('.diff-line.picked')).toHaveLength(3);
    // No comment box while dragging
    expect(composer()).toBeNull();
    // Back up past where it began: from there to the pointer
    await fireEvent.mouseMove(newRun, { clientY: 25 });
    expect(container.querySelectorAll('.diff-line.picked')).toHaveLength(2);
    await fireEvent.mouseUp(window);
    expect(screen.getByText('Lines 7–8')).toBeInTheDocument();
    // Moving on after letting go picks nothing more
    await fireEvent.mouseMove(newRun, { clientY: 65 });
    expect(container.querySelectorAll('.diff-line.picked')).toHaveLength(2);
    await fireEvent.input(composer()!, { target: { value: 'Both lines.' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Comment' }));
    expect(oncomment).toHaveBeenCalledWith({ side: 'new', lines: [7, 8], quote: 'seven\neight a' }, 'Both lines.');
  });

  it('Shift-press extends the pick; a pick on the old side says so; another hunk starts a new pick', async () => {
    const { container } = show();
    await numbers();
    await press(lineOn(container, 'new', 8));
    await press(lineOn(container, 'new', 9), true);
    expect(screen.getByText('Lines 8–9')).toBeInTheDocument();
    expect(container.querySelectorAll('.diff-line.picked')).toHaveLength(2);
    await press(lineOn(container, 'old', 3));
    expect(screen.getByText('Line 3 (before)')).toBeInTheDocument();
    await press(lineOn(container, 'new', 3));
    await press(lineOn(container, 'new', 9), true);
    expect(screen.getByText('Line 9')).toBeInTheDocument();
  });

  it('a pick extends across a chain shown between its lines, and the comment opens after its last line', async () => {
    // The chain on line 3 cuts the first hunk's rows after line 3: lines 2 to 4 are still one pick
    const { container, oncomment } = show({ chains: [chain('t1', 'new', [3, 3], 'Why upper case?')] });
    await numbers();
    await press(lineOn(container, 'new', 2));
    await press(lineOn(container, 'new', 4), true);
    expect(screen.getByText('Lines 2–4')).toBeInTheDocument();
    expect(container.querySelectorAll('.diff-new .diff-line.picked')).toHaveLength(3);
    const form = container.querySelector('.diff-composer')!;
    expect(form.previousElementSibling).toHaveClass('diff-run');
    expect(form.previousElementSibling?.querySelector('[aria-label="Line 4"]')).not.toBeNull();
    await fireEvent.input(composer()!, { target: { value: 'All three.' } });
    await fireEvent.click(within(form as HTMLElement).getByRole('button', { name: 'Comment' }));
    expect(oncomment).toHaveBeenCalledWith({ side: 'new', lines: [2, 4], quote: 'two\nTHREE\nfour' }, 'All three.');
    // Backwards, the same
    await press(lineOn(container, 'new', 4));
    await press(lineOn(container, 'new', 2), true);
    expect(screen.getByText('Lines 2–4')).toBeInTheDocument();
  });

  it('the comment box: empty sends nothing; Cancel and Escape close it; ⌘ Enter sends; busy waits', async () => {
    const { container, oncomment } = show();
    await numbers();
    await press(lineOn(container, 'new', 3));
    expect(screen.getByRole('button', { name: 'Comment' })).toBeDisabled();
    await fireEvent.input(composer()!, { target: { value: '   ' } });
    expect(screen.getByRole('button', { name: 'Comment' })).toBeDisabled();
    await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(composer()).toBeNull();
    expect(container.querySelectorAll('.diff-line.picked')).toHaveLength(0);
    await press(lineOn(container, 'new', 3));
    await fireEvent.keyDown(composer()!, { key: 'Escape' });
    expect(composer()).toBeNull();
    await press(lineOn(container, 'new', 3));
    await fireEvent.input(composer()!, { target: { value: 'Upper?' } });
    await fireEvent.keyDown(composer()!, { key: 'Enter', metaKey: true });
    expect(oncomment).toHaveBeenCalledWith({ side: 'new', lines: [3, 3], quote: 'THREE' }, 'Upper?');
  });

  it('while the page is busy with a change, Comment waits; the comment being written stays', async () => {
    const { container, rerender } = show();
    await numbers();
    await press(lineOn(container, 'new', 3));
    await fireEvent.input(composer()!, { target: { value: 'Upper?' } });
    await rerender({ busy: true });
    expect(container.querySelectorAll('.diff-line.picked')).toHaveLength(1);
    expect(composer()).toHaveValue('Upper?');
    expect(screen.getByRole('button', { name: 'Comment' })).toBeDisabled();
    await rerender({ busy: false });
    expect(screen.getByRole('button', { name: 'Comment' })).toBeEnabled();
  });

  it('without a comment handler, lines are only read: no +, nothing picked', async () => {
    const { container } = show({ oncomment: undefined });
    await numbers();
    const newRun = container.querySelectorAll('.diff-new .diff-code')[1] as HTMLElement;
    await fireEvent.mouseMove(newRun, { clientY: 45 });
    expect(screen.queryByRole('button', { name: /^Comment on line/ })).toBeNull();
    await press(lineOn(container, 'new', 9));
    expect(container.querySelectorAll('.diff-line.picked')).toHaveLength(0);
  });

  it('a right-button press picks nothing; a padding line has no number and no +', async () => {
    const { container } = show();
    await numbers();
    await fireEvent.mouseDown(lineOn(container, 'new', 9), { button: 2 });
    expect(container.querySelectorAll('.diff-line.picked')).toHaveLength(0);
    const oldRun = container.querySelectorAll('.diff-old .diff-code')[1] as HTMLElement;
    // The old side's third line is the padding across from the added line 9
    await fireEvent.mouseMove(oldRun, { clientY: 45 });
    expect(within(oldRun).queryByRole('button', { name: /^Comment on line/ })).toBeNull();
  });
});

describe('DiffView: chains and keys', () => {
  it('a chain shows right after its last line, on its side; replying and resolving are the page’s', async () => {
    const { container, onreply, onresolve } = show({
      chains: [chain('t1', 'new', [3, 3], 'Why upper case?'), chain('t2', 'old', [1, 1], 'A folded line.')],
    });
    await numbers();
    const t1 = container.querySelector('[data-chain="t1"]') as HTMLElement;
    // Right after the run that ends with line 3: the first hunk is cut there
    expect(t1.previousElementSibling).toHaveClass('diff-run');
    expect(t1.querySelector('.chain-where')?.textContent).toContain('Line 3');
    // A chain on a line folded away shows below the file
    expect(container.querySelector('.diff-elsewhere [data-chain="t2"]')).not.toBeNull();
    await fireEvent.click(within(t1).getByRole('button', { name: /Resolve/ }));
    expect(onresolve).toHaveBeenCalledWith('c-t1', true);
    await fireEvent.click(within(t1).getByRole('button', { name: 'Reply' }));
    await fireEvent.input(screen.getByRole('textbox'), { target: { value: 'To match the heading.' } });
    await fireEvent.click(within(t1.querySelector('form')!).getByRole('button', { name: 'Reply' }));
    expect(onreply).toHaveBeenCalledWith('c-t1', 'To match the heading.');
  });

  it('a chain hovered marks its lines on its side; clicked, they stay marked until clicked again', async () => {
    const { container } = show({ chains: [chain('t1', 'new', [7, 9], 'Three lines.')] });
    await numbers();
    const marked = () =>
      [...container.querySelectorAll('.diff-line.commented')].map(
        (l) => `${l.closest('.diff-old') ? 'old' : 'new'}:${l.querySelector('.diff-number')?.textContent}`,
      );
    expect(marked()).toEqual([]);
    const box = container.querySelector('[data-chain="t1"]') as HTMLElement;
    await fireEvent.mouseEnter(box);
    expect(marked()).toEqual(['new:7', 'new:8', 'new:9']);
    await fireEvent.mouseLeave(box);
    expect(marked()).toEqual([]);
    await fireEvent.click(box.querySelector('.chain-body') as HTMLElement);
    await fireEvent.mouseLeave(box);
    expect(marked()).toEqual(['new:7', 'new:8', 'new:9']);
    await fireEvent.click(box.querySelector('.chain-body') as HTMLElement);
    expect(marked()).toEqual([]);
  });

  it('a comment on a line inside another chain’s lines opens right under that line; both chains show, each on its own', async () => {
    const wide = chain('tw', 'new', [7, 9], 'Three lines.');
    const { container, rerender } = show({ chains: [wide] });
    await numbers();
    await press(lineOn(container, 'new', 8));
    const form = container.querySelector('.diff-composer') as HTMLElement;
    // Right after line 8, before the wide chain's box (after line 9)
    expect(form.previousElementSibling?.querySelector('[aria-label="Line 8"]')).not.toBeNull();
    expect(form.nextElementSibling).toHaveClass('diff-run');
    await rerender({ chains: [wide, chain('tn', 'new', [8, 8], 'Just this one.')] });
    expect(container.querySelector('[data-chain="tn"]')).not.toBeNull();
    expect(container.querySelector('[data-chain="tw"]')).not.toBeNull();
  });

  it('a comment the page couldn’t save stays in its box, with why; saved, the box closes', async () => {
    const oncomment = vi.fn().mockResolvedValueOnce('the review is closed').mockResolvedValueOnce(null);
    const { container } = show({ oncomment });
    await numbers();
    await press(lineOn(container, 'new', 3));
    await fireEvent.input(composer()!, { target: { value: 'Keep me.' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Comment' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Not saved: the review is closed');
    expect(composer()).toHaveValue('Keep me.');
    await fireEvent.click(screen.getByRole('button', { name: 'Comment' }));
    await vi.waitFor(() => expect(composer()).toBeNull());
    expect(oncomment).toHaveBeenCalledTimes(2);
  });

  it('keys: n and p go from change to change, ] and [ change file, v marks it viewed; not while typing', async () => {
    const { container, onnextfile, onprevfile, onviewed } = show({
      chains: [chain('t1', 'new', [3, 3], 'Why upper case?')],
    });
    await numbers();
    const scroll = vi.mocked(Element.prototype.scrollIntoView);
    await fireEvent.keyDown(window, { key: 'n' });
    expect((scroll.mock.contexts.at(-1) as HTMLElement).dataset.row).toBe('1-1');
    await fireEvent.keyDown(window, { key: 'n' });
    expect((scroll.mock.contexts.at(-1) as HTMLElement).dataset.row).toBe('3-1');
    await fireEvent.keyDown(window, { key: 'p' });
    expect((scroll.mock.contexts.at(-1) as HTMLElement).dataset.row).toBe('1-1');
    await fireEvent.keyDown(window, { key: ']' });
    await fireEvent.keyDown(window, { key: '[' });
    await fireEvent.keyDown(window, { key: 'v' });
    expect([onnextfile, onprevfile, onviewed].map((f) => f.mock.calls.length)).toEqual([1, 1, 1]);
    // In the chain's reply box, the keys are text
    await fireEvent.click(within(container.querySelector('[data-chain="t1"]')!).getByRole('button', { name: 'Reply' }));
    await fireEvent.keyDown(screen.getByRole('textbox'), { key: 'v' });
    expect(onviewed).toHaveBeenCalledOnce();
  });

  it('keys off: the page has several viewers and the keys itself', async () => {
    const { onnextfile, onviewed } = show({ keyboard: false });
    await numbers();
    await fireEvent.keyDown(window, { key: 'n' });
    await fireEvent.keyDown(window, { key: ']' });
    await fireEvent.keyDown(window, { key: 'v' });
    expect(vi.mocked(Element.prototype.scrollIntoView)).not.toHaveBeenCalled();
    expect([onnextfile, onviewed].map((f) => f.mock.calls.length)).toEqual([0, 0]);
  });
});

describe('DiffView: the rendered view', () => {
  const markdown = {
    before: at('base', '# Plan\n\nOld text.\n', 'markdown'),
    after: at('head', '# Plan\n\nNew text.\n', 'markdown'),
    patch: '@@ -3 +3 @@\n-Old text.\n+New text.\n',
  };

  it('a Markdown file: split shows both sides rendered on the document viewer’s page; unified only the new one', async () => {
    const { container } = show(markdown);
    await numbers();
    await fireEvent.click(screen.getByRole('button', { name: 'Rendered' }));
    expect(container.querySelector('.diff-view')).toHaveAttribute('data-view', 'rendered');
    expect(container.querySelectorAll('.diff-pane')).toHaveLength(2);
    expect(container.querySelectorAll('.diff-pane .doc-page[data-kind="markdown"]')).toHaveLength(2);
    expect(await screen.findByText(/New text\./)).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Unified' }));
    expect(container.querySelectorAll('.diff-pane')).toHaveLength(1);
    expect(container.querySelector('.diff-pane')).toHaveClass('diff-new');
    expect(screen.getByText('After, the changes marked')).toBeInTheDocument();
  });

  it('unified, a Markdown file deleted shows its old side', async () => {
    const { container } = show({ ...markdown, after: null, view: 'rendered', mode: 'unified' });
    await vi.waitFor(() => expect(container.querySelectorAll('.diff-pane')).toHaveLength(1));
    expect(container.querySelector('.diff-pane')).toHaveClass('diff-old');
    expect(screen.getByText('Before (deleted)')).toBeInTheDocument();
  });

  it('a file that isn’t Markdown has no rendered view, even when asked for', async () => {
    const { container } = show({ view: 'rendered' });
    await numbers();
    expect(container.querySelector('.diff-view')).toHaveAttribute('data-view', 'source');
    expect(screen.queryByRole('button', { name: 'Rendered' })).toBeNull();
  });
});
