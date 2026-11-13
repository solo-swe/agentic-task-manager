import { fireEvent, render } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import TitleBarTabs from '../../src/lib/ui/TitleBarTabs.svelte';

type Tab = { id: string; name: string };
const TABS: Tab[] = [
  { id: 'a', name: 'Alpha' },
  { id: 'b', name: 'Beta' },
  { id: 'c', name: 'Gamma' },
];
const label = createRawSnippet((tab: () => Tab) => ({ render: () => `<span class="name">${tab().name}</span>` }));

describe('TitleBarTabs (a reusable title-bar tab strip)', () => {
  it('moves a tab dragged with the pointer, without selecting it; a press that stays put is a click (T-532)', async () => {
    const onmove = vi.fn();
    const onselect = vi.fn();
    const { container } = render(TitleBarTabs<Tab>, {
      tabs: TABS,
      active: 'a',
      label,
      onselect,
      onmove,
      width: 100,
    });
    const tabs = [...container.querySelectorAll<HTMLElement>('[role="tab"]')];
    const pointer = (type: string, el: HTMLElement, clientX: number) =>
      fireEvent(el, new MouseEvent(type, { button: 0, clientX, bubbles: true }));
    // Dragged two tab widths right: it lands third, and the click after the drag doesn't select it
    await pointer('pointerdown', tabs[0], 10);
    await pointer('pointermove', tabs[0], 120);
    // While dragged it follows the pointer; the tab it passed moves aside
    expect(tabs[0].parentElement!.style.transform).toBe('translateX(110px)');
    expect(tabs[1].parentElement!.style.transform).toBe('translateX(-100px)');
    await pointer('pointermove', tabs[0], 205);
    await pointer('pointerup', tabs[0], 205);
    await fireEvent.click(tabs[0]);
    expect(onmove).toHaveBeenCalledWith('a', 2);
    expect(onselect).not.toHaveBeenCalled();
    expect(tabs[0].parentElement!.style.transform).toBe('');
    // A press that moves less than a few pixels is a click
    await pointer('pointerdown', tabs[1], 50);
    await pointer('pointermove', tabs[1], 52);
    await pointer('pointerup', tabs[1], 52);
    await fireEvent.click(tabs[1]);
    expect(onselect).toHaveBeenCalledWith('b');
    expect(onmove).toHaveBeenCalledOnce();
  });

  it('moves the focused tab a place with ⌃⇧ or ⌘⇧ and an arrow, and closes one on a middle click (T-532)', async () => {
    const onmove = vi.fn();
    const onclose = vi.fn();
    const onselect = vi.fn();
    const { container } = render(TitleBarTabs<Tab>, { tabs: TABS, active: 'b', label, onselect, onmove, onclose });
    const tabs = [...container.querySelectorAll<HTMLElement>('[role="tab"]')];
    await fireEvent.keyDown(tabs[1], { key: 'ArrowRight', ctrlKey: true, shiftKey: true });
    await fireEvent.keyDown(tabs[1], { key: 'ArrowLeft', metaKey: true, shiftKey: true });
    // Not past either end
    await fireEvent.keyDown(tabs[0], { key: 'ArrowLeft', ctrlKey: true, shiftKey: true });
    await fireEvent.keyDown(tabs[2], { key: 'ArrowRight', ctrlKey: true, shiftKey: true });
    expect(onmove.mock.calls).toEqual([
      ['b', 2],
      ['b', 0],
    ]);
    expect(onselect).not.toHaveBeenCalled();
    await fireEvent(tabs[1], new MouseEvent('auxclick', { button: 1, bubbles: true }));
    expect(onclose).toHaveBeenCalledWith('b');
  });

  it('draws each tab with the app’s label, the active one selected and focusable', () => {
    const { container } = render(TitleBarTabs<Tab>, { tabs: TABS, active: 'b', label, onselect: () => {} });
    const tabs = [...container.querySelectorAll('[role="tab"]')];
    // Each tab is a button named by its label (drawn over it)
    expect(tabs.map((t) => t.tagName)).toEqual(['BUTTON', 'BUTTON', 'BUTTON']);
    expect(tabs.map((t) => document.getElementById(t.getAttribute('aria-labelledby')!)?.textContent?.trim())).toEqual([
      'Alpha',
      'Beta',
      'Gamma',
    ]);
    expect(tabs.map((t) => t.getAttribute('aria-selected'))).toEqual(['false', 'true', 'false']);
    expect(tabs.map((t) => t.getAttribute('tabindex'))).toEqual(['-1', '0', '-1']);
    expect(tabs[0].id).toBe('title-bar-tab-a');
    // Neither × nor + unless the app handles them
    expect(container.querySelector('.title-bar-tab-close')).toBeNull();
    expect(container.querySelector('.title-bar-tab-new')).toBeNull();
  });

  it('selects on a click and with the keyboard', async () => {
    const onselect = vi.fn();
    const { container } = render(TitleBarTabs<Tab>, { tabs: TABS, active: 'b', label, onselect });
    const tabs = [...container.querySelectorAll<HTMLElement>('[role="tab"]')];
    await fireEvent.click(tabs[2]);
    await fireEvent.keyDown(tabs[1], { key: 'ArrowLeft' });
    await fireEvent.keyDown(tabs[1], { key: 'ArrowRight' });
    await fireEvent.keyDown(tabs[1], { key: 'End' });
    await fireEvent.keyDown(tabs[1], { key: 'Home' });
    expect(onselect.mock.calls.map(([id]) => id)).toEqual(['c', 'a', 'c', 'c', 'a']);
  });

  it('closes a tab without selecting it, and opens a new one', async () => {
    const onselect = vi.fn();
    const onclose = vi.fn();
    const onnew = vi.fn();
    const { container } = render(TitleBarTabs<Tab>, {
      tabs: TABS,
      active: 'a',
      label,
      onselect,
      onclose,
      onnew,
      idPrefix: 'x-',
      width: 180,
    });
    await fireEvent.click(container.querySelectorAll('.title-bar-tab-close')[1]);
    expect(onclose).toHaveBeenCalledWith('b');
    expect(onselect).not.toHaveBeenCalled();
    await fireEvent.click(container.querySelector('#x-new')!);
    expect(onnew).toHaveBeenCalledOnce();
    expect(container.querySelector<HTMLElement>('.title-bar-tabs')!.style.getPropertyValue('--tab-width')).toBe(
      '180px',
    );
  });
});
