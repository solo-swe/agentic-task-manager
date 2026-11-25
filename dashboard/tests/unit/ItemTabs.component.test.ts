import { fireEvent, render } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import { ALL_ITEMS, ITEMS, type Item } from '../../src/lib/address';
import ItemTabs from '../../src/lib/shell/ItemTabs.svelte';
import SubTabs from '../../src/lib/shell/SubTabs.svelte';

// The page's items as tabs, and an item's views as sub-tabs (ATM-752, docs/sidebar-overhaul-design.md §5)

const names = (container: HTMLElement, selector: string) =>
  [...container.querySelectorAll(selector)].map((b) => b.textContent?.trim());

describe('ItemTabs', () => {
  it('a project’s six items in the sidebar’s order, the current one selected and the only one in the tab order', () => {
    const { container } = render(ItemTabs, { items: ITEMS, current: 'tasks', onselect: () => {}, label: 'Project' });
    expect(names(container, '[role=tab]')).toEqual([
      'Overview',
      'Inbox',
      'Tasks',
      'Code Reviews',
      'Documents',
      'Analysis',
    ]);
    const tasks = container.querySelector('#item-tasks')!;
    expect(tasks).toHaveAttribute('aria-selected', 'true');
    expect(tasks).toHaveAttribute('tabindex', '0');
    expect(container.querySelector('#item-overview')).toHaveAttribute('tabindex', '-1');
    expect(container.querySelector('[role=tablist]')).toHaveAttribute('aria-label', 'Project');
  });

  it('All projects’ four: no Inbox or Documents', () => {
    const { container } = render(ItemTabs, { items: ALL_ITEMS, current: 'overview', onselect: () => {}, label: 'All' });
    expect(names(container, '[role=tab]')).toEqual(['Overview', 'Tasks', 'Code Reviews', 'Analysis']);
  });

  it('a click selects the item; the arrow keys the next or previous one, wrapping, and move the focus', async () => {
    const onselect = vi.fn<(item: Item) => void>();
    const { container } = render(ItemTabs, { items: ITEMS, current: 'overview', onselect, label: 'Project' });
    await fireEvent.click(container.querySelector('#item-reviews')!);
    expect(onselect).toHaveBeenLastCalledWith('reviews');
    await fireEvent.keyDown(container.querySelector('#item-overview')!, { key: 'ArrowLeft' });
    expect(onselect).toHaveBeenLastCalledWith('analysis');
    expect(document.activeElement).toBe(container.querySelector('#item-analysis'));
    await fireEvent.keyDown(container.querySelector('#item-overview')!, { key: 'ArrowRight' });
    expect(onselect).toHaveBeenLastCalledWith('inbox');
    onselect.mockClear();
    await fireEvent.keyDown(container.querySelector('#item-overview')!, { key: 'Enter' });
    expect(onselect).not.toHaveBeenCalled();
  });

  it('a badge with a count names it; none without one', () => {
    const { container } = render(ItemTabs, {
      items: ITEMS,
      current: 'overview',
      onselect: () => {},
      label: 'Project',
      badges: {
        reviews: { count: 2, says: 'waiting for you', id: 'reviews-waiting' },
        inbox: { count: 0, says: 'new' },
      },
    });
    expect(container.querySelector('#reviews-waiting')).toHaveTextContent('2');
    expect(container.querySelector('#item-reviews')).toHaveAttribute('aria-label', 'Code Reviews, 2 waiting for you');
    expect(container.querySelector('#item-inbox .view-tab-badge')).toBeNull();
    expect(container.querySelector('#item-inbox')).not.toHaveAttribute('aria-label');
  });
});

describe('SubTabs', () => {
  const views = [
    { id: 'kanban', label: 'Board', icon: 'square-kanban' },
    { id: 'list', label: 'List', icon: 'list-todo' },
    { id: 'graph', label: 'Dependencies', icon: 'network' },
  ] as const;

  it('the item’s views, with their ids and panels, the current one selected', () => {
    const { container } = render(SubTabs, {
      views,
      current: 'list',
      onselect: () => {},
      idPrefix: 'tab-',
      panel: (id: string) => `panel-${id}`,
      label: 'Tasks',
    });
    expect(names(container, '.sub-tab')).toEqual(['Board', 'List', 'Dependencies']);
    expect(container.querySelector('#tab-list')).toHaveAttribute('aria-selected', 'true');
    expect(container.querySelector('#tab-graph')).toHaveAttribute('aria-controls', 'panel-graph');
    expect(container.querySelector('[role=tablist]')).toHaveAttribute('aria-label', 'Tasks');
  });

  it('a click or an arrow key selects a view, wrapping at the ends', async () => {
    const onselect = vi.fn();
    const { container } = render(SubTabs, {
      views,
      current: 'kanban',
      onselect,
      idPrefix: 'tab-',
      panel: () => 'p',
      label: 'Tasks',
    });
    await fireEvent.click(container.querySelector('#tab-graph')!);
    expect(onselect).toHaveBeenLastCalledWith('graph');
    await fireEvent.keyDown(container.querySelector('#tab-kanban')!, { key: 'ArrowLeft' });
    expect(onselect).toHaveBeenLastCalledWith('graph');
    expect(document.activeElement).toBe(container.querySelector('#tab-graph'));
  });
});
