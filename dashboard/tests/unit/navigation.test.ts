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
    expect(history.state).toEqual({ atmFrom: '/', atmDocument: true });
    expect(currentAddress()).toBe('/projects/abc/list?task=T-1');
    navigate('/board', { replace: true });
    expect([location.pathname, history.length]).toEqual(['/board', before + 1]);
    // Replaced, the entry keeps where it was reached from, and nothing else of its state
    expect(entryState()).toEqual({ atmFrom: '/' });
  });

  it('going to the address shown adds no entry; replacing it still writes it', () => {
    navigate('/board');
    const before = history.length;
    navigate('/board');
    expect(history.length).toBe(before);
    navigate('/board', { replace: true, state: { atmDocumentDepth: 2 } });
    expect(entryState()).toEqual({ atmFrom: '/', atmDocumentDepth: 2 });
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
      ['/projects/abc', { replace: false, state: { atmFrom: '/' } }],
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
