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
