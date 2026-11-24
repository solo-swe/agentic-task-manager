import { describe, expect, it } from 'vitest';
import { ITEMS } from '../../src/lib/address';
import { itemAddress, itemsOf, scopeFromPref, scopePref, sidebarScope } from '../../src/lib/sidebar';

// The app's sidebar: its scope and items (lib/sidebar.ts, ATM-753, docs/sidebar-overhaul-design.md §2)

describe('itemsOf', () => {
  it('a project has six items, All projects four (no Inbox or Documents)', () => {
    expect(itemsOf('atm')).toEqual(ITEMS);
    expect(itemsOf(null)).toEqual(['overview', 'tasks', 'reviews', 'analysis']);
  });
});

describe('itemAddress', () => {
  it.each([
    ['atm', 'overview', '/projects/atm/project'],
    ['atm', 'inbox', '/projects/atm/inbox'],
    ['atm', 'tasks', '/projects/atm/kanban'],
    ['atm', 'reviews', '/projects/atm/reviews'],
    ['atm', 'documents', '/projects/atm/documents'],
    ['atm', 'analysis', '/projects/atm/progress'],
    [null, 'overview', '/projects'],
    [null, 'tasks', '/board'],
    [null, 'reviews', '/reviews'],
    [null, 'analysis', '/progress'],
  ] as const)('%s’s %s opens on its first view: %s', (scope, item, address) => {
    expect(itemAddress('default', scope, item)).toBe(address);
  });

  it('in another workspace, under it; a project’s id is encoded', () => {
    expect(itemAddress('work', 'atm', 'tasks')).toBe('/w/work/projects/atm/kanban');
    expect(itemAddress('work', null, 'overview')).toBe('/w/work/projects');
    expect(itemAddress('', 'a b', 'reviews')).toBe('/projects/a%20b/reviews');
  });

  it('All projects has no Inbox or Documents', () => {
    expect(itemAddress('default', null, 'inbox')).toBeNull();
    expect(itemAddress('default', null, 'documents')).toBeNull();
  });
});

describe('sidebarScope', () => {
  it('follows the active tab’s page: a project’s, or All’s', () => {
    expect(sidebarScope('atm', 't1', null, null)).toBe('atm');
    expect(sidebarScope(null, 't1', null, 'atm')).toBeNull();
  });

  it('a page of no scope (the landing page, the new tab) keeps the last one', () => {
    expect(sidebarScope(undefined, 't1', null, 'atm')).toBe('atm');
    expect(sidebarScope(undefined, 't1', null, null)).toBeNull();
  });

  it('a choice by hand holds while the active tab and its page’s scope stay', () => {
    const choice = { scope: 'lfb', tab: 't1', pageScope: 'atm' };
    expect(sidebarScope('atm', 't1', choice, 'atm')).toBe('lfb');
    // Another tab, or the page moved to another scope: it follows again
    expect(sidebarScope('atm', 't2', choice, 'lfb')).toBe('atm');
    expect(sidebarScope(null, 't1', choice, 'lfb')).toBeNull();
    // All chosen by hand on a project's page
    expect(sidebarScope('atm', 't1', { scope: null, tab: 't1', pageScope: 'atm' }, 'atm')).toBeNull();
  });
});

describe('the scope in the preferences', () => {
  const projects = [{ id: 'atm' }, { id: 'gone', missing: true }];

  it('a project by its id, All as an empty string; back, a project no longer listed or missing is All', () => {
    expect(scopePref('atm')).toBe('atm');
    expect(scopePref(null)).toBe('');
    expect(scopeFromPref('atm', projects)).toBe('atm');
    expect(scopeFromPref('', projects)).toBeNull();
    expect(scopeFromPref('gone', projects)).toBeNull();
    expect(scopeFromPref('other', projects)).toBeNull();
  });
});
