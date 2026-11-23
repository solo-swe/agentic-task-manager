import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  backEntry,
  currentAddress,
  entryState,
  navigate,
  originOf,
  type EntryState,
  type Router,
} from '../../src/lib/navigation';

// Moving between addresses (lib/navigation.ts, T-429): through the router the layout hands in, else the browser's
// history; the address written last is the current one until the router arrives.

/** A router that records where it was sent and keeps its own entry state */
function fakeRouter(state: EntryState = {}) {
  const went: [string, { replace: boolean; state: EntryState }][] = [];
  const backs: number[] = [];
  const router: Router = {
    go: (address, options) => void went.push([address, options]),
    state: () => state,
    back: (steps) => void backs.push(steps),
  };
  return { router, went, backs };
}

/** Navigation's module state is its own: a fresh copy per test */
async function fresh() {
  vi.resetModules();
  return import('../../src/lib/navigation');
}

beforeEach(() => history.replaceState(null, '', '/'));
afterEach(() => history.replaceState(null, '', '/'));

describe('without a router (the browser’s history)', () => {
  it('pushes a new entry with its state, or replaces this one; the address is the location’s at once', () => {
    const before = history.length;
    navigate('/projects/abc/list?task=T-1', { state: { atmDocument: true } });
    expect([location.pathname + location.search, history.length]).toEqual(['/projects/abc/list?task=T-1', before + 1]);
    expect(history.state).toEqual({ atmFrom: '/', atmPrev: '/', atmDocument: true });
    expect(currentAddress()).toBe('/projects/abc/list?task=T-1');
    navigate('/board', { replace: true });
    expect([location.pathname, history.length]).toEqual(['/board', before + 1]);
    // Replaced, the entry keeps where it was reached from and the entry before it, and nothing else of its state
    expect(entryState()).toEqual({ atmFrom: '/', atmPrev: '/' });
  });

  it('going to the address shown adds no entry; replacing it still writes it', () => {
    navigate('/board');
    const before = history.length;
    navigate('/board');
    expect(history.length).toBe(before);
    navigate('/board', { replace: true, state: { atmDocumentDepth: 2 } });
    expect(entryState()).toEqual({ atmFrom: '/', atmPrev: '/', atmDocumentDepth: 2 });
  });

  it('the current address is the location as the dashboard writes it (an old #… link’s new one)', () => {
    history.replaceState(null, '', '/#project=abc&view=list');
    expect(currentAddress()).toBe('/projects/abc/list');
    history.replaceState(null, '', '/projects/abc/');
    expect(currentAddress()).toBe('/projects/abc');
  });

  it('an entry without state has none', () => {
    history.replaceState(null, '', '/');
    expect(entryState()).toEqual({});
  });
});

describe('with the layout’s router', () => {
  it('hands it each address, with replace and state; the address is the one written until it arrives', async () => {
    const nav = await fresh();
    const { router, went } = fakeRouter({ atmDocumentDepth: 1 });
    nav.useRouter(router);
    nav.navigate('/projects/abc');
    nav.navigate('/projects/abc/graph', { replace: true, state: { atmDocument: true } });
    expect(went).toEqual([
      ['/projects/abc', { replace: false, state: { atmFrom: '/', atmPrev: '/' } }],
      ['/projects/abc/graph', { replace: true, state: { atmDocument: true } }],
    ]);
    // The router hasn't moved the location yet: the address is the one written
    expect(nav.currentAddress()).toBe('/projects/abc/graph');
    expect(nav.entryState()).toEqual({ atmDocumentDepth: 1 });
    nav.arrived();
    expect(nav.currentAddress()).toBe('/');
  });

  it('going again to the address just written, not yet arrived, sends nothing', async () => {
    const nav = await fresh();
    const { router, went } = fakeRouter();
    nav.useRouter(router);
    nav.navigate('/board');
    nav.navigate('/board');
    expect(went).toHaveLength(1);
    nav.navigate('/board', { replace: true });
    expect(went).toHaveLength(2);
  });

  it('the back button goes as many entries back as the page was reached from', async () => {
    const nav = await fresh();
    const { router, backs } = fakeRouter({ atmFrom: '/projects/abc', atmSteps: 3 });
    nav.useRouter(router);
    expect(nav.backEntry()).toEqual({ address: '/projects/abc', steps: 3 });
    nav.goBack();
    expect(backs).toEqual([3]);
  });

  it('with no page it was reached from, the back button goes nowhere', async () => {
    const nav = await fresh();
    const { router, backs } = fakeRouter();
    nav.useRouter(router);
    expect(nav.backEntry()).toBeNull();
    nav.goBack();
    expect(backs).toEqual([]);
  });
});

// Where an entry was reached from (ATM-686): what a review's back button goes back to and names
describe('where an entry was reached from', () => {
  it('a push remembers the page before, one step back', () => {
    navigate('/projects/abc/kanban');
    navigate('/projects/abc/reviews/CR-1');
    expect(backEntry()).toEqual({ address: '/projects/abc/kanban', steps: 1 });
  });

  it('a task’s details opened from the board are a place to come back to', () => {
    navigate('/projects/abc/kanban?task=ATM-12');
    navigate('/projects/abc/reviews/CR-1');
    expect(backEntry()).toEqual({ address: '/projects/abc/kanban?task=ATM-12', steps: 1 });
  });

  it('on the same page (details over it, another file or round), the page’s own way back, a step further', () => {
    navigate('/projects/abc/kanban');
    navigate('/projects/abc/reviews/CR-1');
    navigate('/projects/abc/reviews/CR-1?task=ATM-12');
    expect(backEntry()).toEqual({ address: '/projects/abc/kanban', steps: 2 });
    // The details closed (replacing the entry) and another round chosen: still the board, three back
    navigate('/projects/abc/reviews/CR-1', { replace: true });
    navigate('/projects/abc/reviews/CR-1?round=2');
    expect(backEntry()).toEqual({ address: '/projects/abc/kanban', steps: 3 });
  });

  it('a page opened on its own has no way back, also after something opened over it', () => {
    history.replaceState(null, '', '/projects/abc/reviews/CR-1');
    navigate('/projects/abc/reviews/CR-1?task=ATM-12');
    expect(backEntry()).toBeNull();
  });

  it('another of the app’s tabs shown (fresh) has no way back in its tab', () => {
    navigate('/projects/abc/kanban');
    navigate('/projects/abc/reviews/CR-1', { fresh: true });
    expect(backEntry()).toBeNull();
  });

  it('originOf: another page is reached from the one shown; the same page from where it was', () => {
    expect(originOf('/b', '/a?x=1', {})).toEqual({ atmFrom: '/a?x=1' });
    expect(originOf('/a?y=2', '/a?x=1', { atmFrom: '/z' })).toEqual({ atmFrom: '/z', atmSteps: 2 });
    expect(originOf('/a?y=2', '/a?x=1', { atmFrom: '/z', atmSteps: 2 })).toEqual({ atmFrom: '/z', atmSteps: 3 });
    expect(originOf('/a?y=2', '/a', {})).toEqual({});
  });
});

// ATM-694 (docs/tabs-and-navigation-design.md §4.1, §4.3): in the app each title-bar tab has its own history; the
// page's own stays one entry deep
describe('a tab’s own history (the app)', () => {
  /** The app: a fresh navigation module, the tabs as its history, a router whose entry is the last one it was sent */
  async function app(start = '/projects/abc/kanban') {
    const nav = await fresh();
    const { AppTabs } = await import('../../src/lib/app-tabs.svelte');
    const { tabActions } = await import('../../src/lib/tab-actions');
    const tabs = new AppTabs();
    tabs.start(start);
    let state: EntryState = {};
    let shown = start;
    const went: [string, boolean][] = [];
    nav.useRouter({
      go: (address, { replace, state: next }) => {
        went.push([address, replace]);
        shown = address;
        state = next;
        // As the app's router: the location moves there, its one entry replaced
        history.replaceState(null, '', address);
        nav.arrived();
      },
      state: () => state,
      back: () => {
        throw new Error('the page’s own history is never walked in the app');
      },
    });
    history.replaceState(null, '', start);
    nav.useTabHistory(tabs);
    const actions = tabActions(tabs, {
      go: () => nav.showTab(),
      leave: nav.rememberScroll,
      closeWindow: () => {},
      openWindow: async () => true,
    });
    return { nav, tabs, actions, went, shown: () => shown };
  }

  const addresses = (tabs: { current?: { history: { entries: { address: string }[]; at: number } } }) => {
    const h = tabs.current!.history;
    return [h.entries.map((e) => e.address), h.at];
  };

  it('a step adds to the active tab’s history; Back and Forward walk it; the page’s own entry is always replaced', async () => {
    const { nav, tabs, went, shown } = await app();
    expect(nav.canGo(-1)).toBe(false);
    nav.navigate('/projects/abc/kanban?task=T-1');
    nav.navigate('/projects/abc/reviews/CR-7');
    expect(addresses(tabs)).toEqual([
      ['/projects/abc/kanban', '/projects/abc/kanban?task=T-1', '/projects/abc/reviews/CR-7'],
      2,
    ]);
    expect(went.every(([, replace]) => replace)).toBe(true);
    // The review page's back button knows where it was reached from
    expect(nav.backEntry()).toEqual({ address: '/projects/abc/kanban?task=T-1', steps: 1 });
    expect(nav.back()).toBe(true);
    expect(shown()).toBe('/projects/abc/kanban?task=T-1');
    expect(nav.back()).toBe(true);
    expect(shown()).toBe('/projects/abc/kanban');
    expect([nav.canGo(-1), nav.back()]).toEqual([false, false]);
    expect(nav.forward()).toBe(true);
    expect(shown()).toBe('/projects/abc/kanban?task=T-1');
    // A step from the middle drops what was ahead, as a browser's does
    nav.navigate('/projects/abc/list');
    expect(addresses(tabs)).toEqual([
      ['/projects/abc/kanban', '/projects/abc/kanban?task=T-1', '/projects/abc/list'],
      2,
    ]);
    expect(nav.canGo(1)).toBe(false);
  });

  // ATM-696: closing what is open over a page never leaves that page twice in the tab's history
  it('closing what was opened over the page under it goes Back to it; another task’s details in place too', async () => {
    const { nav, tabs, shown } = await app('/projects/abc/list');
    nav.navigate('/projects/abc/kanban');
    nav.navigate('/projects/abc/kanban?task=T-1');
    nav.navigate('/projects/abc/kanban?task=T-2', { replace: true });
    nav.closeTo('/projects/abc/kanban');
    expect(shown()).toBe('/projects/abc/kanban');
    expect(addresses(tabs)).toEqual([
      ['/projects/abc/list', '/projects/abc/kanban', '/projects/abc/kanban?task=T-2'],
      1,
    ]);
    // Back from there: the page before, not the board again
    nav.back();
    expect(shown()).toBe('/projects/abc/list');
  });

  it('closing what was opened on its own (a link, a new tab) replaces its entry with the page', async () => {
    const { nav, tabs, shown } = await app('/projects/abc/kanban?task=T-1');
    nav.closeTo('/projects/abc/kanban');
    expect(shown()).toBe('/projects/abc/kanban');
    expect(addresses(tabs)).toEqual([['/projects/abc/kanban'], 0]);
    // Opened from another page: that page isn't the one under it
    nav.navigate('/projects/abc/list?task=T-2');
    nav.closeTo('/projects/abc/list');
    expect(addresses(tabs)).toEqual([['/projects/abc/kanban', '/projects/abc/list'], 1]);
  });

  it('a replace changes the current entry only', async () => {
    const { nav, tabs } = await app();
    nav.navigate('/projects/abc/list');
    nav.navigate('/projects/abc/list?sprint=S1', { replace: true });
    expect(addresses(tabs)).toEqual([['/projects/abc/kanban', '/projects/abc/list?sprint=S1'], 1]);
  });

  it('switching tabs adds nothing to either history; each shows as it was left', async () => {
    const { nav, tabs, actions, shown } = await app();
    nav.navigate('/projects/abc/reviews/CR-7');
    const first = tabs.active;
    actions.open('/board');
    expect(shown()).toBe('/board');
    expect(nav.canGo(-1)).toBe(false); // a new tab's Back is disabled until it moves
    nav.navigate('/board?task=T-2');
    actions.select(first);
    expect(shown()).toBe('/projects/abc/reviews/CR-7');
    expect(addresses(tabs)).toEqual([['/projects/abc/kanban', '/projects/abc/reviews/CR-7'], 1]);
    nav.back();
    expect(shown()).toBe('/projects/abc/kanban');
    actions.step(1);
    expect(shown()).toBe('/board?task=T-2');
    expect(addresses(tabs)).toEqual([['/board', '/board?task=T-2'], 1]);
  });

  it('each entry keeps the scroll of its page, restored on Back and on a switch back to its tab', async () => {
    const { nav, actions } = await app();
    const board = document.body.appendChild(document.createElement('div'));
    board.id = 'kanban-board';
    try {
      board.scrollTop = 120;
      board.scrollLeft = 30;
      // jsdom keeps what is set: what the page would scroll to
      expect(nav.scrollOf()).toEqual({ 'kanban-board': { top: 120, left: 30 } });
      nav.navigate('/projects/abc/list');
      board.scrollTop = 0;
      board.scrollLeft = 0;
      nav.back();
      expect([board.scrollTop, board.scrollLeft]).toEqual([120, 30]);
      // Left for another tab and back
      board.scrollTop = 64;
      actions.open('/board');
      board.scrollTop = 0;
      actions.step(-1);
      expect(board.scrollTop).toBe(64);
    } finally {
      board.remove();
    }
  });
});
