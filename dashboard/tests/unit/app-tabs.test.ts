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

describe('a tab’s own history (ATM-694)', () => {
  it('tabs an older page kept, without histories, start one at their address; a broken one too', () => {
    const storage = memory();
    storage.setItem(
      'atm-app-tabs',
      JSON.stringify({
        tabs: [
          { id: 't1', address: '/board' },
          { id: 't2', address: '/projects/p1', history: { entries: [], at: 3 } },
        ],
        active: 't1',
      }),
    );
    const tabs = new AppTabs(storage);
    tabs.start('/board');
    expect(tabs.tabs.map((t) => t.history)).toEqual([
      { entries: [{ address: '/board', state: {} }], at: 0 },
      { entries: [{ address: '/projects/p1', state: {} }], at: 0 },
    ]);
  });

  it('keeps its last 50 entries; Back and Forward stop at its ends; kept over a reload', () => {
    const storage = memory();
    const tabs = new AppTabs(storage);
    tabs.start('/');
    for (let i = 1; i <= 60; i++) tabs.push({ address: `/projects/p${i}`, state: {} });
    expect(tabs.current!.history.entries).toHaveLength(50);
    expect(tabs.current!.history.entries[0].address).toBe('/projects/p11');
    expect(tabs.go(1)).toBeNull();
    expect(tabs.go(-49)?.address).toBe('/projects/p11');
    expect([tabs.canGo(-1), tabs.go(-1), tabs.current!.address]).toEqual([false, null, '/projects/p11']);
    const reloaded = new AppTabs(storage);
    reloaded.start('/projects/p11');
    expect(reloaded.current!.history.at).toBe(0);
    expect(reloaded.canGo(1)).toBe(true);
  });
});

describe('site data blocked (ATM-676, audit D5)', () => {
  it('the module loads when reading sessionStorage throws; its tabs work, kept nowhere', async () => {
    const real = Object.getOwnPropertyDescriptor(window, 'sessionStorage');
    Object.defineProperty(window, 'sessionStorage', {
      configurable: true,
      get() {
        throw new DOMException('blocked', 'SecurityError');
      },
    });
    try {
      vi.resetModules();
      const module = await import('../../src/lib/app-tabs.svelte');
      expect(module.sessionStore()).toBeNull();
      module.appTabs.start('/projects/p1');
      expect(module.appTabs.tabs.map((t) => t.address)).toEqual(['/projects/p1']);
    } finally {
      Object.defineProperty(window, 'sessionStorage', real!);
      vi.resetModules();
    }
  });
});

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

  it('comes back with the tabs the app kept (T-518): the active one to go to; the app’s Reload too', () => {
    const storage = memory();
    const tabs = new AppTabs(storage);
    expect(tabs.start('/', { addresses: ['/projects/a', '/board'], active: 1 })).toBe('/board');
    expect(addresses(tabs)).toEqual(['/projects/a', '/board']);
    expect(tabs.kept).toEqual({ addresses: ['/projects/a', '/board'], active: 1 });
    tabs.select(tabs.tabs[0].id);
    expect(tabs.kept.active).toBe(0);
    // The page reloaded by itself (the app says nothing): its own tabs, the active one where the page is
    const reloaded = new AppTabs(storage);
    expect(reloaded.start('/projects/a/list', null)).toBeNull();
    expect(addresses(reloaded)).toEqual(['/projects/a/list', '/board']);
    // The app's Reload (ATM-584) loads the page at the app's root and says what the window showed: those, to go to
    const viaApp = new AppTabs(storage);
    expect(viaApp.start('/', { addresses: ['/projects/a/list', '/board'], active: 1 })).toBe('/board');
    expect(addresses(viaApp)).toEqual(['/projects/a/list', '/board']);
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

  it('a new tab goes to the chooser of scope and item (ATM-755); select, step and close go where the tab is', () => {
    const { tabs, actions, went } = setup();
    actions.open();
    actions.open('/projects/b');
    actions.select(tabs.tabs[0].id);
    actions.step(1);
    actions.close();
    expect(went).toEqual(['/new', '/projects/b', '/projects/a', '/new', '/projects/b']);
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
