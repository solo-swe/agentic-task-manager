import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import ChainBox from '../../src/lib/reviews/ChainBox.svelte';
import type { Chain, ReviewComment } from '../../src/lib/reviews/types';

// A chain of comments (lib/reviews/ChainBox.svelte, ATM-573, ATM-574): where it is, where it stands (Resolve while
// open, a resolved check above the comment once resolved, folded until shown), replies. CompMD doesn't load here.
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
  const handlers = { onreply: vi.fn(), onresolve: vi.fn() };
  const result = render(ChainBox, { chain: c, ...handlers, ...more });
  return { ...result, ...handlers };
}

beforeEach(() => {
  vi.spyOn(console, 'error').mockImplementation(() => {});
  loadCompmd.mockRejectedValue(new Error('no CompMD in these tests'));
});
afterEach(() => vi.restoreAllMocks());

describe('ChainBox: where it is', () => {
  it.each([
    [{}, 'src/a.rs:3–5'],
    [{ lines: [4, 4] as [number, number] }, 'src/a.rs:4'],
    [{ lines: null }, 'src/a.rs'],
    [{ side: 'old' as const }, 'src/a.rs:3–5 (before)'],
    [{ path: null, lines: null }, 'The review'],
  ])('%o: %s', (anchor, where) => {
    const { container } = show(chain([comment('c1')], anchor));
    expect(container.querySelector('.chain-where')?.textContent?.trim()).toBe(where);
  });

  it('with onwhere, its place is a link; the quote shows under it', async () => {
    const onwhere = vi.fn();
    show(chain([comment('c1')], { quote: 'let x = 1;' }), { onwhere });
    await fireEvent.click(screen.getByRole('button', { name: 'src/a.rs:3–5' }));
    expect(onwhere).toHaveBeenCalledOnce();
    expect(screen.getByText('let x = 1;')).toBeTruthy();
  });
});

describe('ChainBox: where it stands', () => {
  it('open: a Resolve button above the comment', async () => {
    const { container, onresolve } = show(chain([comment('c1')]));
    const state = container.querySelector('[data-comment="c1"] > .chain-state') as HTMLElement;
    expect(state.nextElementSibling).toHaveClass('chain-meta');
    await fireEvent.click(within(state).getByRole('button', { name: /Resolve/ }));
    expect(onresolve).toHaveBeenCalledWith('c1', true);
    expect(container.querySelector('.chain')).not.toHaveAttribute('data-resolved');
  });

  it('resolved: folded to its place and a check, until shown; then the check above the comment, and Reopen', async () => {
    const { container, onresolve } = show(chain([comment('c1', { resolved: true })], { quote: 'q' }));
    expect(container.querySelector('.chain')).toHaveAttribute('data-resolved');
    expect(container.querySelector('.chain-folded-note')?.textContent?.replace(/\s+/g, ' ').trim()).toBe(
      '✓ Resolved · Ana',
    );
    expect(container.querySelector('[data-comment]')).toBeNull();
    expect(screen.queryByText('q')).toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: 'Show' }));
    const state = container.querySelector('[data-comment="c1"] > .chain-state.resolved') as HTMLElement;
    expect(state.textContent).toContain('Resolved');
    await fireEvent.click(within(state).getByRole('button', { name: 'Reopen' }));
    expect(onresolve).toHaveBeenCalledWith('c1', false);
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

describe('ChainBox: replies', () => {
  it('Reply opens a box under that comment; an empty reply isn’t sent; Cancel closes it', async () => {
    const { container, onreply } = show(chain([comment('c1', { replies: [comment('r1', { resolved: null })] })]));
    const reply = within(container.querySelector('[data-comment="r1"]') as HTMLElement);
    await fireEvent.click(reply.getAllByRole('button', { name: 'Reply' })[0]);
    const box = screen.getByRole('textbox', { name: 'Reply to Ana' });
    expect(container.querySelector('[data-comment="r1"] form')).not.toBeNull();
    const send = within(container.querySelector('form') as HTMLElement).getByRole('button', { name: 'Reply' });
    expect(send).toBeDisabled();
    await fireEvent.input(box, { target: { value: '  ' } });
    expect(send).toBeDisabled();
    await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(container.querySelector('form')).toBeNull();
    expect(onreply).not.toHaveBeenCalled();
  });

  it('a reply sent goes to the page, trimmed, and the box closes; Reply again toggles it shut', async () => {
    const { container, onreply } = show(chain([comment('c1')]));
    const toggle = screen.getByRole('button', { name: 'Reply' });
    await fireEvent.click(toggle);
    await fireEvent.click(toggle);
    expect(container.querySelector('form')).toBeNull();
    await fireEvent.click(toggle);
    await fireEvent.input(screen.getByRole('textbox'), { target: { value: ' Fixed in round 2. ' } });
    await fireEvent.submit(container.querySelector('form') as HTMLElement);
    expect(onreply).toHaveBeenCalledWith('c1', 'Fixed in round 2.');
    expect(container.querySelector('form')).toBeNull();
  });
});
