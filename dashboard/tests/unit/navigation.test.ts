import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { currentAddress, entryState, navigate, type EntryState, type Router } from '../../src/lib/navigation';

// Moving between addresses (lib/navigation.ts, T-429): through the router the layout hands in, else the browser's
// history; the address written last is the current one until the router arrives.

/** A router that records where it was sent and keeps its own entry state */
function fakeRouter(state: EntryState = {}) {
  const went: [string, { replace: boolean; state: EntryState }][] = [];
  const router: Router = { go: (address, options) => void went.push([address, options]), state: () => state };
  return { router, went };
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
    expect(history.state).toEqual({ atmDocument: true });
    expect(currentAddress()).toBe('/projects/abc/list?task=T-1');
    navigate('/board', { replace: true });
    expect([location.pathname, history.length]).toEqual(['/board', before + 1]);
    expect(entryState()).toEqual({});
  });

  it('going to the address shown adds no entry; replacing it still writes it', () => {
    navigate('/board');
    const before = history.length;
    navigate('/board');
    expect(history.length).toBe(before);
    navigate('/board', { replace: true, state: { atmDocumentDepth: 2 } });
    expect(entryState()).toEqual({ atmDocumentDepth: 2 });
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
      ['/projects/abc', { replace: false, state: {} }],
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
});
