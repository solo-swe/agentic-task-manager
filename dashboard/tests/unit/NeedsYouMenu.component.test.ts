import { fireEvent, render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import NeedsYouMenu from '../../src/lib/shell/NeedsYouMenu.svelte';
import type { NeedsYouItem } from '../../src/lib/waiting';
import { press, settled } from './components.helpers';

const ITEMS: NeedsYouItem[] = [
  { id: 'T-005', name: 'Post editor', reason: 'review' },
  { id: 'T-008', name: 'Editor keyboard shortcuts', reason: 'blocked' },
  { id: 'T-010', name: 'Search', reason: 'blocked' },
];

function setup(items: NeedsYouItem[], extra: Record<string, unknown> = {}) {
  const onopen = vi.fn();
  const view = render(NeedsYouMenu, { props: { items, onopen, chipId: 'chip', menuId: 'menu', ...extra } });
  const chip = view.container.querySelector<HTMLButtonElement>('#chip');
  const menu = () => view.container.querySelector('#menu');
  const rows = () => [...view.container.querySelectorAll<HTMLButtonElement>('.needs-you-row')];
  return { ...view, onopen, chip, menu, rows };
}

/** A key as the keyboard sends it: to what has the focus */
const key = (name: string) => fireEvent.keyDown(document.activeElement!, { key: name });

// The list is Bits UI's menu (T-431): it opens and moves focus a moment after the click, and closes once its layer has
// let go of the page
describe('NeedsYouMenu', () => {
  it('is not there when nothing needs the user', () => {
    expect(setup([]).chip).toBeNull();
  });

  it('one task: the chip opens it, with no list', async () => {
    const { chip, menu, onopen } = setup(ITEMS.slice(0, 1));
    expect(chip).toHaveTextContent('1 needs you');
    expect(chip!.getAttribute('aria-haspopup')).toBeNull();
    await fireEvent.click(chip!);
    expect(menu()).toBeNull();
    expect(onopen).toHaveBeenCalledWith('T-005');
  });

  it('several: the chip lists them with why; a row opens that task and closes the list', async () => {
    const { chip, menu, rows, onopen } = setup(ITEMS);
    expect(chip).toHaveTextContent('3 need you');
    expect(chip!.getAttribute('aria-haspopup')).toBe('menu');
    await fireEvent.click(chip!);
    expect(chip!.getAttribute('aria-expanded')).toBe('true');
    expect(menu()).toHaveAttribute('role', 'menu');
    expect(rows().map((r) => r.dataset.id)).toEqual(['T-005', 'T-008', 'T-010']);
    expect(rows()[1]).toHaveAttribute('role', 'menuitem');
    expect(rows()[1]).toHaveTextContent('T-008 Editor keyboard shortcuts blocked on a question');
    expect(rows()[0].querySelector('.needs-you-reason')).toHaveTextContent('in review');
    await settled();
    await fireEvent.click(rows()[1]);
    expect(onopen).toHaveBeenCalledWith('T-008');
    await vi.waitFor(() => expect(menu()).toBeNull());
  });

  it('keyboard: the arrows, Home and End move; Escape closes and returns focus to the chip', async () => {
    const { chip, menu, rows, onopen } = setup(ITEMS);
    chip!.focus();
    await fireEvent.keyDown(chip!, { key: 'Enter' });
    const [a, b, c] = rows();
    await vi.waitFor(() => expect(document.activeElement).toBe(a));
    await key('ArrowDown');
    expect(document.activeElement).toBe(b);
    await key('End');
    expect(document.activeElement).toBe(c);
    await key('Home');
    expect(document.activeElement).toBe(a);
    await key('Escape');
    await vi.waitFor(() => expect(menu()).toBeNull());
    await vi.waitFor(() => expect(document.activeElement).toBe(chip));
    expect(onopen).not.toHaveBeenCalled();
  });

  it('closes on a click outside, and with the chip', async () => {
    const { chip, menu } = setup(ITEMS.slice(0, 2));
    await fireEvent.click(chip!);
    await settled();
    await press(document.body);
    await vi.waitFor(() => expect(menu()).toBeNull());
    await fireEvent.click(chip!);
    expect(menu()).not.toBeNull();
    await settled();
    await fireEvent.click(chip!);
    await vi.waitFor(() => expect(menu()).toBeNull());
  });

  it('lines up with the right edge when asked (on a project card)', async () => {
    const { chip, menu } = setup(ITEMS, { align: 'right', chipClass: 'project-card-need' });
    expect(chip).toHaveClass('attention-chip', 'project-card-need');
    await fireEvent.click(chip!);
    expect(menu()).toHaveClass('align-right');
  });

  it('closes when fewer than two tasks are left to list', async () => {
    const { chip, menu, rerender } = setup(ITEMS.slice(0, 2));
    await fireEvent.click(chip!);
    await rerender({ items: ITEMS.slice(0, 1) });
    await tick();
    expect(menu()).toBeNull();
  });
});
