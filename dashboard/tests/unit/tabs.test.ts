import { describe, expect, it } from 'vitest';
import { ALL_ITEMS, itemOf, ITEMS, OVERVIEW_TABS } from '../../src/lib/address';
import { withProject } from '../../src/lib/routes';
import {
  adjacent,
  ALL_ITEM_VIEWS,
  DEFAULT_VIEW,
  itemOfOverviewTab,
  itemOfView,
  namesView,
  PROJECT_ITEM_VIEWS,
  viewOf,
  VIEWS,
  withView,
} from '../../src/lib/tabs';

describe('viewOf', () => {
  it("reads the project's tab from its address (T-429)", () => {
    expect(viewOf('/projects/p1/kanban')).toBe('kanban');
    expect(viewOf('/projects/p1/progress?about')).toBe('progress');
    expect(viewOf('/?view=list')).toBe('list');
    expect(namesView('/projects/p1/list')).toBe(true);
    expect(namesView('/projects/p1')).toBe(false);
  });

  it('falls back to the Overview, the Project tab (ATM-751: no longer the Kanban, T-251)', () => {
    expect(DEFAULT_VIEW).toBe('project');
    expect(viewOf('')).toBe('project');
    expect(viewOf('/')).toBe('project');
    expect(viewOf('/projects/p1')).toBe('project');
    expect(viewOf('/projects/p1/nope')).toBe('project');
    expect(viewOf('/projects/p1/graph')).toBe('graph');
  });
});

describe('withView', () => {
  it('sets the view and keeps other parameters', () => {
    expect(withView('/projects/p1', 'kanban')).toBe('/projects/p1/kanban');
    expect(withView('/projects/p1/graph', 'progress')).toBe('/projects/p1/progress');
    expect(withView('/projects/p1?about', 'kanban')).toBe('/projects/p1/kanban?about');
  });

  it('round-trips through viewOf', () => {
    for (const v of VIEWS) expect(viewOf(withView('/projects/p1?x=y', v))).toBe(v);
  });
});

describe('adjacent', () => {
  it('moves left/right and wraps; from a value not in the list, from the first', () => {
    const items = ['a', 'b', 'c'];
    expect(adjacent(items, 'a', 1)).toBe('b');
    expect(adjacent(items, 'c', 1)).toBe('a');
    expect(adjacent(items, 'a', -1)).toBe('c');
    expect(adjacent(items, 'x', 1)).toBe('b');
  });
});

// The views by the sidebar's items (ATM-752, docs/sidebar-overhaul-design.md §1)
describe('the items’ views', () => {
  it('a project’s views are its items’ in order; each view is in exactly one item', () => {
    expect(ITEMS.flatMap((item) => PROJECT_ITEM_VIEWS[item])).toEqual([...VIEWS]);
    expect(PROJECT_ITEM_VIEWS.tasks).toEqual(['kanban', 'list', 'graph']);
    expect(PROJECT_ITEM_VIEWS.analysis).toEqual(['progress', 'agents']);
    for (const view of VIEWS) expect(PROJECT_ITEM_VIEWS[itemOfView(view)]).toContain(view);
  });

  it('All projects has four items, Tasks with the Board and the List; every overview tab is in one', () => {
    expect(Object.keys(ALL_ITEM_VIEWS)).toEqual(ALL_ITEMS);
    expect(ALL_ITEM_VIEWS.tasks).toEqual(['board', 'list']);
    expect(ALL_ITEM_VIEWS.analysis).toEqual(['progress', 'agents']);
    expect(OVERVIEW_TABS.map(itemOfOverviewTab)).toEqual([
      'overview',
      'tasks',
      'tasks',
      'analysis',
      'analysis',
      'reviews',
    ]);
  });

  it('a view’s item is the one address.ts gives its address', () => {
    for (const view of VIEWS) expect(itemOf(`/projects/p1/${view}`)).toBe(itemOfView(view));
    for (const tab of OVERVIEW_TABS) expect(itemOf(withProject('/', null, tab))).toBe(itemOfOverviewTab(tab));
  });
});
