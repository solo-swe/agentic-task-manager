import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import DiffView from '../../src/lib/reviews/DiffView.svelte';
import type { DocumentResult, ProjectDocument } from '../../src/lib/documents';
import type { DocumentSource } from '../../src/lib/documents/source';
import type { Chain } from '../../src/lib/reviews/types';

// The diff viewer (docs/code-review-design.md §3.7, ATM-573): a round's file, its sides from document sources made up
// here (CompMD doesn't load in these tests, so each run of lines shows as written, with the viewer's gutter beside it)
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

/** One side: the round's base or head, at its revision */
const at = (id: string, text: string | null, kind?: ProjectDocument['kind']): DocumentSource => ({
  id,
  load: async (path): Promise<DocumentResult> =>
    text === null
      ? { unchanged: false, error: 'missing', message: `${path}: not in ${id}` }
      : { unchanged: false, document: doc(path, text, kind), validators: {} },
  imageUrl: () => null,
  repositoryLink: () => null,
});

const chain = (id: string, side: 'old' | 'new', lines: [number, number], body: string): Chain => ({
  id,
  anchor: { round: 1, path: 'f.txt', side, lines, quote: null },
  comments: [{ id: `c-${id}`, author: 'Hakan Serce', at: '2026-11-17T10:00:00Z', body, resolved: false, replies: [] }],
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

beforeEach(() => {
  vi.spyOn(console, 'error').mockImplementation(() => {});
  loadCompmd.mockRejectedValue(new Error('no CompMD in these tests'));
  Element.prototype.scrollIntoView = vi.fn();
});

afterEach(() => {
  vi.restoreAllMocks();
  loadCompmd.mockReset();
});

describe('DiffView', () => {
  it('split: each side’s lines, the changes marked, the unchanged runs folded until shown', async () => {
    const { container } = show();
    await numbers();
    expect(container.querySelectorAll('.diff-run')).toHaveLength(2);
    // Line 3 changed (marked on both sides), line 9 only added (a padding line on the old side)
    expect(container.querySelectorAll('.diff-changed')).toHaveLength(4);
    expect(container.querySelectorAll('.diff-new .diff-added')).toHaveLength(1);
    expect(container.querySelectorAll('.diff-old .diff-pad')).toHaveLength(1);
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
    expect(container.querySelectorAll('.diff-removed')).toHaveLength(2);
    expect(container.querySelectorAll('.diff-added')).toHaveLength(3);
    expect(container.querySelector('.diff-code.both')).not.toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: 'Split' }));
    expect(container.querySelector('.diff-view')).toHaveAttribute('data-mode', 'split');
  });

  it('a click on a line picks it, Shift extends the pick; Comment hands the lines and their text to the page', async () => {
    const { container, oncomment } = show();
    await numbers();
    const newSide = container.querySelectorAll('.diff-new')[1] as HTMLElement;
    const [, eightA, eightB] = within(newSide).getAllByRole('button', { name: /^Line \d+$/ });
    await fireEvent.click(eightA);
    await fireEvent.click(eightB, { shiftKey: true });
    expect(screen.getByText('Lines 8–9')).toBeInTheDocument();
    expect(container.querySelectorAll('.diff-line.picked')).toHaveLength(2);
    await fireEvent.click(screen.getByRole('button', { name: 'Comment' }));
    expect(oncomment).toHaveBeenCalledWith({ side: 'new', lines: [8, 9], quote: 'eight a\neight b' });
    // A pick on the old side says so
    const oldSide = container.querySelectorAll('.diff-old')[0] as HTMLElement;
    await fireEvent.click(within(oldSide).getAllByRole('button', { name: /^Line \d+$/ })[1]);
    expect(screen.getByText('Line 3 (before)')).toBeInTheDocument();
  });

  it('a pick extends across a chain shown between its lines, and Comment shows after its last line', async () => {
    // The chain on line 3 cuts the first hunk's rows after line 3: lines 2 to 4 are still one pick
    const { container, oncomment } = show({ chains: [chain('t1', 'new', [3, 3], 'Why upper case?')] });
    await numbers();
    const lineOn = (n: number) =>
      [...container.querySelectorAll('.diff-new')]
        .flatMap((side) => within(side as HTMLElement).queryAllByRole('button', { name: `Line ${n}` }))
        .at(0)!;
    await fireEvent.click(lineOn(2));
    await fireEvent.click(lineOn(4), { shiftKey: true });
    expect(screen.getByText('Lines 2–4')).toBeInTheDocument();
    expect(container.querySelectorAll('.diff-new .diff-line.picked')).toHaveLength(3);
    // The Comment button after the last line picked, past the chain
    const picked = container.querySelector('.diff-picked')!;
    expect(picked.previousElementSibling).toHaveClass('diff-run');
    expect(picked.previousElementSibling?.querySelector('[aria-label="Line 4"]')).not.toBeNull();
    await fireEvent.click(within(picked as HTMLElement).getByRole('button', { name: 'Comment' }));
    expect(oncomment).toHaveBeenCalledWith({ side: 'new', lines: [2, 4], quote: 'two\nTHREE\nfour' });
    // Backwards, the same
    await fireEvent.click(lineOn(4));
    await fireEvent.click(lineOn(2), { shiftKey: true });
    expect(screen.getByText('Lines 2–4')).toBeInTheDocument();
    // Another side, or another hunk, starts a new pick
    await fireEvent.click(lineOn(9), { shiftKey: true });
    expect(screen.getByText('Line 9')).toBeInTheDocument();
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

  it('a chain shows right after its last line, on its side; replying and resolving are the page’s', async () => {
    const { container, onreply, onresolve } = show({
      chains: [chain('t1', 'new', [3, 3], 'Why upper case?'), chain('t2', 'old', [1, 1], 'A folded line.')],
    });
    await numbers();
    const t1 = container.querySelector('[data-chain="t1"]')!;
    // Right after the run that ends with line 3: the first hunk is cut there
    expect(t1.previousElementSibling).toHaveClass('diff-run');
    expect(t1.textContent).toContain('f.txt:3');
    // A chain on a line folded away shows below the file
    expect(container.querySelector('.diff-elsewhere [data-chain="t2"]')).not.toBeNull();
    await fireEvent.click(within(t1 as HTMLElement).getByRole('checkbox'));
    expect(onresolve).toHaveBeenCalledWith('c-t1', true);
    await fireEvent.click(within(t1 as HTMLElement).getByRole('button', { name: 'Reply' }));
    await fireEvent.input(screen.getByRole('textbox'), { target: { value: 'To match the heading.' } });
    await fireEvent.click(within(t1.querySelector('form')!).getByRole('button', { name: 'Reply' }));
    expect(onreply).toHaveBeenCalledWith('c-t1', 'To match the heading.');
  });

  it('a new file has no old side; a side that can’t be read says why', async () => {
    const { container, unmount } = show({
      before: null,
      patch: '@@ -0,0 +1,2 @@\n+a\n+b\n',
      after: at('head', 'a\nb\n'),
    });
    await numbers();
    expect(container.querySelectorAll('.diff-new .diff-added')).toHaveLength(2);
    expect(container.querySelectorAll('.diff-old .diff-pad')).toHaveLength(2);
    unmount();
    show({ after: at('head', null) });
    expect(await screen.findByRole('alert')).toHaveTextContent('f.txt: not in head');
  });

  it('a Markdown file offers the rendered view: each side rendered whole', async () => {
    const { container } = show({
      before: at('base', '# Plan\n\nOld text.\n', 'markdown'),
      after: at('head', '# Plan\n\nNew text.\n', 'markdown'),
      patch: '@@ -3 +3 @@\n-Old text.\n+New text.\n',
    });
    await numbers();
    await fireEvent.click(screen.getByRole('button', { name: 'Rendered' }));
    expect(container.querySelector('.diff-view')).toHaveAttribute('data-view', 'rendered');
    expect(container.querySelectorAll('.diff-pane')).toHaveLength(2);
    expect(await screen.findByText(/New text\./)).toBeInTheDocument();
  });
});
