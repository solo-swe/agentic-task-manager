import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import ChainBox from '../../src/lib/reviews/ChainBox.svelte';
import type { Chain, ReviewComment } from '../../src/lib/reviews/types';

// A chain of comments (lib/reviews/ChainBox.svelte, ATM-573, ATM-574): where it is (its lines; `detailed`, its file
// too, and the text it was made on), where it stands (Resolve while open, a resolved check above the comment once
// resolved, folded until shown, Reopen at hand either way), replies, and selecting it. CompMD doesn't load here.
const loadCompmd = vi.hoisted(() => vi.fn());
vi.mock('../../src/lib/compmd', async (actual) => ({ ...(await actual()), loadCompmd }));

const comment = (id: string, more: Partial<ReviewComment> = {}): ReviewComment => ({
  id,
  author: 'Ana',
  at: '2026-11-17T10:00:00Z',
  body: `Body of ${id}`,
  resolved: false,
  replies: [],
  ...more,
});

const chain = (comments: ReviewComment[], anchor: Partial<Chain['anchor']> = {}): Chain => ({
  id: 't1',
  anchor: { round: 1, path: 'src/a.rs', side: 'new', lines: [3, 5], quote: null, ...anchor },
  comments,
});

function show(c: Chain, more: Record<string, unknown> = {}) {
  const handlers = { onreply: vi.fn(), onresolve: vi.fn(), onselect: vi.fn() };
  const result = render(ChainBox, { chain: c, ...handlers, ...more });
  return { ...result, ...handlers };
}
const whereOf = (container: HTMLElement) => container.querySelector('.chain-where > :first-child')?.textContent?.trim();

beforeEach(() => {
  vi.spyOn(console, 'error').mockImplementation(() => {});
  loadCompmd.mockRejectedValue(new Error('no CompMD in these tests'));
});
afterEach(() => vi.restoreAllMocks());

describe('ChainBox: where it is', () => {
  it.each([
    [{}, 'Lines 3–5'],
    [{ lines: [4, 4] as [number, number] }, 'Line 4'],
    [{ lines: null }, 'The file'],
    [{ side: 'old' as const }, 'Lines 3–5 · before'],
    [{ path: null, lines: null }, 'The review'],
  ])('in the diff, its lines alone (the diff shows the file and the text): %o is %s', (anchor, where) => {
    const { container } = show(chain([comment('c1')], { ...anchor, quote: 'the text' }));
    expect(whereOf(container)).toBe(where);
    expect(screen.queryByText('the text')).toBeNull();
  });

  it.each([
    [{}, 'src/a.rs · Lines 3–5'],
    [{ lines: null }, 'src/a.rs'],
    [{ side: 'old' as const, lines: [2, 2] as [number, number] }, 'src/a.rs · Line 2 · before'],
    [{ path: null, lines: null }, 'The review'],
  ])('with context (the Comments tab), its file too: %o is %s', (anchor, where) => {
    const { container } = show(chain([comment('c1')], anchor), { detailed: true });
    expect(whereOf(container)).toBe(where);
  });

  it('with context and onwhere, its place is a link and the text it was made on shows', async () => {
    const onwhere = vi.fn();
    show(chain([comment('c1')], { quote: 'let x = 1;' }), { detailed: true, onwhere });
    await fireEvent.click(screen.getByRole('button', { name: 'src/a.rs · Lines 3–5' }));
    expect(onwhere).toHaveBeenCalledOnce();
    expect(screen.getByText('let x = 1;')).toBeTruthy();
  });

  it('each comment with its author’s initials, name and time', () => {
    const { container } = show(chain([comment('c1', { author: 'Hakan Serce' })]));
    expect(container.querySelector('.chain-avatar')?.textContent).toBe('HS');
    expect(container.querySelector('.chain-author')?.textContent).toBe('Hakan Serce');
    expect(container.querySelector('time')).toHaveAttribute('datetime', '2026-11-17T10:00:00Z');
    expect(container.querySelector('time')?.textContent).not.toBe('');
  });
});

describe('ChainBox: where it stands', () => {
  // ATM-611: a poke is marked by its author, with nothing to resolve; one with no message says it was a poke
  it('a poke: marked, no Resolve, never folded; an empty one reads Poked', () => {
    const poke = (id: string, body: string) => comment(id, { resolved: null, poke: true, body });
    const { container } = show(chain([poke('c1', 'Where are we?'), poke('c2', '')], { path: null, lines: null }));
    expect(container.querySelectorAll('.chain-poke')).toHaveLength(2);
    expect(container.querySelector('.chain-resolve')).toBeNull();
    expect(container.querySelector('.chain')).not.toHaveAttribute('data-resolved');
    expect(container.querySelector('[data-comment="c2"] .chain-poke-empty')).toHaveTextContent('Poked');
  });

  it('open: a Resolve button above the comment', async () => {
    const { container, onresolve } = show(chain([comment('c1')]));
    const state = container.querySelector('[data-comment="c1"] > .chain-state') as HTMLElement;
    expect(state.nextElementSibling).toHaveClass('chain-meta');
    await fireEvent.click(within(state).getByRole('button', { name: /Resolve/ }));
    expect(onresolve).toHaveBeenCalledWith('c1', true);
    expect(container.querySelector('.chain')).not.toHaveAttribute('data-resolved');
  });

  it('resolved: folded to its place and a check, Reopen at hand; shown, the check above the comment, and Reopen', async () => {
    const { container, onresolve } = show(chain([comment('c1', { resolved: true })]));
    expect(container.querySelector('.chain')).toHaveAttribute('data-resolved');
    expect(container.querySelector('.chain-folded-note')?.textContent?.replace(/\s+/g, ' ').trim()).toBe(
      'Resolved · Ana',
    );
    expect(container.querySelector('[data-comment]')).toBeNull();
    // Reopened from the fold
    await fireEvent.click(screen.getByRole('button', { name: 'Reopen' }));
    expect(onresolve).toHaveBeenLastCalledWith('c1', false);
    await fireEvent.click(screen.getByRole('button', { name: 'Show' }));
    const state = container.querySelector('[data-comment="c1"] > .chain-state.resolved') as HTMLElement;
    expect(state.textContent).toContain('Resolved');
    await fireEvent.click(within(state).getByRole('button', { name: 'Reopen' }));
    expect(onresolve).toHaveBeenCalledTimes(2);
    await fireEvent.click(screen.getByRole('button', { name: 'Hide' }));
    expect(container.querySelector('[data-comment]')).toBeNull();
  });

  it('a chain with one comment open of two isn’t resolved; replies have no box of their own', () => {
    const { container } = show(
      chain([comment('c1', { resolved: true, replies: [comment('r1', { resolved: null })] }), comment('c2')]),
    );
    expect(container.querySelector('.chain')).not.toHaveAttribute('data-resolved');
    expect(container.querySelectorAll('.chain-state')).toHaveLength(2);
    expect(container.querySelector('[data-comment="r1"] > .chain-state')).toBeNull();
    expect(container.querySelector('[data-comment="r1"]')).toHaveAttribute('data-depth', '1');
  });

  it('without handlers, only read: the check shows, no Resolve, Reopen or Reply', async () => {
    const { container } = render(ChainBox, {
      chain: chain([comment('c1', { resolved: true }), comment('c2', { resolved: true })]),
    });
    await fireEvent.click(screen.getByRole('button', { name: 'Show' }));
    expect(container.querySelectorAll('.chain-state.resolved')).toHaveLength(2);
    expect(screen.queryByRole('button', { name: /Resolve|Reopen|Reply/ })).toBeNull();
  });
});

describe('ChainBox: selecting it', () => {
  it('hovered or focused, it is selected while so; clicked, kept; a click on its buttons isn’t a select', async () => {
    const { container, onselect } = show(chain([comment('c1')]));
    const box = container.querySelector('.chain') as HTMLElement;
    await fireEvent.mouseEnter(box);
    await fireEvent.mouseLeave(box);
    await fireEvent.focusIn(box);
    await fireEvent.click(container.querySelector('.chain-body') as HTMLElement);
    expect(onselect.mock.calls).toEqual([
      [true, false],
      [false, false],
      [true, false],
      [true, true],
    ]);
    await fireEvent.click(screen.getByRole('button', { name: /Resolve/ }));
    expect(onselect).toHaveBeenCalledTimes(4);
  });

  it('selected, it says so', () => {
    const { container } = show(chain([comment('c1')]), { selected: true });
    expect(container.querySelector('.chain')).toHaveClass('selected');
  });
});

describe('ChainBox: replies', () => {
  it('Reply opens a box under that comment; an empty reply isn’t sent; Cancel and Escape close it', async () => {
    const { container, onreply } = show(chain([comment('c1', { replies: [comment('r1', { resolved: null })] })]));
    const reply = within(container.querySelector('[data-comment="r1"]') as HTMLElement);
    await fireEvent.click(reply.getByRole('button', { name: 'Reply' }));
    const box = screen.getByRole('textbox', { name: 'Reply to Ana' });
    expect(box).toHaveFocus();
    expect(container.querySelector('[data-comment="r1"] form')).not.toBeNull();
    const send = within(container.querySelector('form') as HTMLElement).getByRole('button', { name: 'Reply' });
    expect(send).toBeDisabled();
    await fireEvent.input(box, { target: { value: '  ' } });
    expect(send).toBeDisabled();
    await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(container.querySelector('form')).toBeNull();
    await fireEvent.click(reply.getByRole('button', { name: 'Reply' }));
    await fireEvent.keyDown(screen.getByRole('textbox'), { key: 'Escape' });
    expect(container.querySelector('form')).toBeNull();
    expect(onreply).not.toHaveBeenCalled();
  });

  it('a reply sent goes to the page, trimmed, and the box closes; ⌘ Enter sends too', async () => {
    const { container, onreply } = show(chain([comment('c1')]));
    await fireEvent.click(screen.getByRole('button', { name: 'Reply' }));
    await fireEvent.input(screen.getByRole('textbox'), { target: { value: ' Fixed in round 2. ' } });
    await fireEvent.submit(container.querySelector('form') as HTMLElement);
    expect(onreply).toHaveBeenCalledWith('c1', 'Fixed in round 2.');
    expect(container.querySelector('form')).toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: 'Reply' }));
    await fireEvent.input(screen.getByRole('textbox'), { target: { value: 'Again.' } });
    await fireEvent.keyDown(screen.getByRole('textbox'), { key: 'Enter', metaKey: true });
    expect(onreply).toHaveBeenLastCalledWith('c1', 'Again.');
  });
});
