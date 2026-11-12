import { describe, expect, it, vi } from 'vitest';
import { AppTabs, tabProject } from '../../src/lib/app-tabs.svelte';
import { tabActions, type TabWindow } from '../../src/lib/tab-actions';

/** A storage in memory, as sessionStorage is the window's */
function memory(): Storage {
  const items = new Map<string, string>();
  return {
    get length() {
      return items.size;
    },
    clear: () => items.clear(),
    getItem: (k) => items.get(k) ?? null,
    key: (i) => [...items.keys()][i] ?? null,
    removeItem: (k) => void items.delete(k),
    setItem: (k, v) => void items.set(k, v),
  };
}

const addresses = (tabs: AppTabs) => tabs.tabs.map((t) => t.address);

describe('the app’s tabs (T-514): a tab is an address', () => {
  it('starts with one tab at the page shown, and the active tab follows the page', () => {
    const tabs = new AppTabs();
    tabs.start('/projects/a/kanban');
    expect(addresses(tabs)).toEqual(['/projects/a/kanban']);
    tabs.follow('/projects/a/list?task=T-1');
    expect(tabs.current?.address).toBe('/projects/a/list?task=T-1');
  });

  it('opens a new tab after the active one, and steps round the ends', () => {
    const tabs = new AppTabs();
    tabs.start('/projects/a');
    expect(tabs.open('/projects/b')).toBe('/projects/b');
    tabs.select(tabs.tabs[0].id);
    tabs.open('/projects/c');
    expect(addresses(tabs)).toEqual(['/projects/a', '/projects/c', '/projects/b']);
    expect(tabs.step(1)).toBe('/projects/b');
    expect(tabs.step(1)).toBe('/projects/a');
    expect(tabs.step(-1)).toBe('/projects/b');
    expect(tabs.select('nope')).toBeNull();
  });

  it('closing the active tab goes to the next (or the one before); closing the last one says so', () => {
    const tabs = new AppTabs();
    tabs.start('/projects/a');
    tabs.open('/projects/b');
    tabs.open('/projects/c');
    tabs.select(tabs.tabs[1].id);
    expect(tabs.close(tabs.tabs[1].id)).toBe('/projects/c');
    expect(tabs.close(tabs.tabs[0].id)).toBeUndefined(); // not the active one: nothing to go to
    expect(addresses(tabs)).toEqual(['/projects/c']);
    expect(tabs.close('nope')).toBeUndefined();
    expect(tabs.close(tabs.tabs[0].id)).toBeNull();
  });

  it('moves a tab, and IDs are never reused', () => {
    const tabs = new AppTabs();
    tabs.start('/a');
    tabs.open('/b');
    tabs.open('/c');
    tabs.move(tabs.tabs[2].id, 0);
    expect(addresses(tabs)).toEqual(['/c', '/a', '/b']);
    tabs.move(tabs.tabs[0].id, 99);
    expect(addresses(tabs)).toEqual(['/a', '/b', '/c']);
    const ids = tabs.tabs.map((t) => t.id);
    tabs.close(tabs.active);
    tabs.open('/d');
    expect(ids).not.toContain(tabs.active);
  });

  it('keeps the tabs over a reload, at the page shown', () => {
    const storage = memory();
    const before = new AppTabs(storage);
    before.start('/projects/a');
    before.open('/projects/b');
    const after = new AppTabs(storage);
    after.start('/projects/b/list');
    expect(addresses(after)).toEqual(['/projects/a', '/projects/b/list']);
    after.open('/x');
    expect(new Set(after.tabs.map((t) => t.id)).size).toBe(3);
    // Something unreadable kept: a fresh start
    storage.setItem('atm-app-tabs', '{');
    const fresh = new AppTabs(storage);
    fresh.start('/');
    expect(addresses(fresh)).toEqual(['/']);
  });

  it('comes back with the tabs the app kept (T-518): the active one to go to; a reload keeps its own', () => {
    const storage = memory();
    const tabs = new AppTabs(storage);
    expect(tabs.start('/', { addresses: ['/projects/a', '/board'], active: 1 })).toBe('/board');
    expect(addresses(tabs)).toEqual(['/projects/a', '/board']);
    expect(tabs.kept).toEqual({ addresses: ['/projects/a', '/board'], active: 1 });
    tabs.select(tabs.tabs[0].id);
    expect(tabs.kept.active).toBe(0);
    // A reload: the window's own tabs, whatever the app says
    const reloaded = new AppTabs(storage);
    expect(reloaded.start('/projects/a', { addresses: ['/x'], active: 0 })).toBeNull();
    expect(addresses(reloaded)).toEqual(['/projects/a', '/board']);
    // Nothing kept, or an active place out of range
    expect(new AppTabs().start('/', null)).toBeNull();
    const out = new AppTabs();
    expect(out.start('/', { addresses: ['/a', '/b'], active: 9 })).toBe('/a');
  });

  it('names a tab’s project from its address', () => {
    expect(tabProject('/projects/abc/list?task=T-1')).toBe('abc');
    expect(tabProject('/w/work/projects/abc')).toBe('abc');
    expect(tabProject('/')).toBeNull();
  });
});

describe('what the tabs do (T-514)', () => {
  function setup() {
    const went: string[] = [];
    const win: TabWindow = {
      go: (address) => went.push(address),
      closeWindow: vi.fn(),
      openWindow: vi.fn(async () => true),
    };
    const tabs = new AppTabs();
    tabs.start('/projects/a');
    return { tabs, actions: tabActions(tabs, win), win, went };
  }

  it('a new tab goes to the overview; select, step and close go where the tab is', () => {
    const { tabs, actions, went } = setup();
    actions.open();
    actions.open('/projects/b');
    actions.select(tabs.tabs[0].id);
    actions.step(1);
    actions.close();
    expect(went).toEqual(['/', '/projects/b', '/projects/a', '/', '/projects/b']);
  });

  it('closing the last tab closes the window', () => {
    const { actions, win } = setup();
    actions.close();
    expect(win.closeWindow).toHaveBeenCalled();
  });

  it('Move Tab to New Window opens one at the tab’s address, and the tab leaves', async () => {
    const { tabs, actions, win, went } = setup();
    actions.open('/projects/b');
    actions.toWindow();
    await Promise.resolve();
    expect(win.openWindow).toHaveBeenCalledWith('/projects/b');
    expect(addresses(tabs)).toEqual(['/projects/a']);
    expect(went.at(-1)).toBe('/projects/a');
  });
});
