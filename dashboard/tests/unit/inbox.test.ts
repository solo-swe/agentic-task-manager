import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { inboxListOf, withInboxList } from '../../src/lib/address';
import { inboxUrl, type Fetch } from '../../src/lib/api/http';
import { addItem, discardItem, editItem, loadInbox, restoreItem } from '../../src/lib/api/inbox';
import { callFor } from '../../src/lib/api/ipc';
import { setApiWorkspace } from '../../src/lib/api/workspace';

// The inbox's client (lib/api/inbox.ts, ATM-762), its address in the page's (lib/address.ts) and in the app's commands
// (lib/api/ipc.ts). The transport is stubbed: fetch, the IPC.
const where = vi.hoisted(() => ({ app: false }));
vi.mock('../../src/lib/api/http', async (actual) => ({
  ...(await actual()),
  get inApp() {
    return where.app;
  },
}));

const TOKEN = 'b'.repeat(64);
function withToken(token: string | null) {
  document.head.querySelector('meta[name="atm-session"]')?.remove();
  if (token === null) return;
  const meta = document.createElement('meta');
  meta.name = 'atm-session';
  meta.content = token;
  document.head.prepend(meta);
}

beforeEach(() => {
  where.app = false;
  withToken(TOKEN);
});
afterEach(() => {
  vi.unstubAllGlobals();
  clearMocks();
  setApiWorkspace('default');
  withToken(null);
});

describe('the inbox’s address', () => {
  it('the route: the open items by default, a state and the archive when given', () => {
    expect(inboxUrl('p1')).toBe('/api/v1/projects/p1/inbox');
    expect(inboxUrl('p1', 'all', true)).toBe('/api/v1/projects/p1/inbox?state=all&archived=1');
    setApiWorkspace('w');
    expect(inboxUrl('p1', 'discarded')).toBe('/api/v2/workspaces/w/projects/p1/inbox?state=discarded');
  });

  it('the page’s filter: read with its defaults (an unknown state is Open), written without them', () => {
    expect(inboxListOf('/projects/p1/inbox')).toEqual({ state: 'created', archived: false });
    expect(inboxListOf('/projects/p1/inbox?state=injected&archived=1')).toEqual({ state: 'injected', archived: true });
    expect(inboxListOf('/projects/p1/inbox?state=bogus&archived=2')).toEqual({ state: 'created', archived: false });
    expect(withInboxList('/projects/p1/inbox?task=T-1', { state: 'all', archived: true })).toBe(
      '/projects/p1/inbox?task=T-1&state=all&archived=1',
    );
    expect(withInboxList('/projects/p1/inbox?state=all&archived=1', { state: 'created', archived: false })).toBe(
      '/projects/p1/inbox',
    );
  });

  it('the app’s command for the route: its state and the archive', () => {
    expect(callFor('/api/v1/projects/p1/inbox')).toEqual({
      command: 'project_inbox',
      args: { workspace: 'default', project: 'p1', which: undefined, archived: false },
    });
    expect(callFor('/api/v2/workspaces/w/projects/p1/inbox?state=all&archived=1')!.args).toEqual({
      workspace: 'w',
      project: 'p1',
      which: 'all',
      archived: true,
    });
  });
});

describe('loadInbox', () => {
  const answering = (body: unknown, status = 200) =>
    vi.fn(async () => new Response(JSON.stringify(body), { status, headers: { ETag: '"i1"' } })) as unknown as Fetch;

  it('the items and the counts; anything unexpected left out', async () => {
    const result = await loadInbox(
      'p1',
      'all',
      false,
      answering({
        items: [{ id: 'IN-2', text: 'two', state: 'created' }, { id: 3 }, null, { id: 'IN-1', text: 'one' }],
        created: 2,
        discarded: -1,
        injected: 'x',
      }),
    );
    expect(result).toEqual({
      unchanged: false,
      listing: {
        items: [
          { id: 'IN-2', text: 'two', state: 'created' },
          { id: 'IN-1', text: 'one' },
        ],
        created: 2,
        discarded: 0,
        injected: 0,
      },
      validators: { etag: '"i1"' },
    });
  });

  it('asked again with its validators: a 304 is unchanged; an error rejects', async () => {
    const fetch = vi.fn(async () => new Response(null, { status: 304 })) as unknown as Fetch;
    expect(await loadInbox('p1', 'created', false, fetch, { etag: '"i1"' })).toEqual({ unchanged: true });
    await expect(loadInbox('p1', 'created', false, answering({}, 500))).rejects.toThrow('HTTP 500');
  });
});

describe('the inbox’s changes', () => {
  it('in the browser, each is its route under the project’s inbox, with the page’s token, as JSON', async () => {
    const fetch = vi.fn<(url: string, init?: RequestInit) => Promise<Response>>(
      async () => new Response(JSON.stringify({ item: { id: 'IN-1' } }), { status: 200 }),
    );
    vi.stubGlobal('fetch', fetch);
    await addItem('p1', 'A note');
    await editItem('p1', 'IN-1', 'Better');
    await discardItem('p1', 'IN-1', 'Not now.');
    await restoreItem('p1', 'IN-1');
    const inbox = '/api/v2/workspaces/default/projects/p1/inbox';
    expect(fetch.mock.calls.map(([url, init]) => [url, init!.method, JSON.parse(String(init!.body))])).toEqual([
      [inbox, 'POST', { text: 'A note' }],
      [`${inbox}/IN-1`, 'PATCH', { text: 'Better' }],
      [`${inbox}/IN-1/decisions`, 'POST', { decision: 'discarded', reason: 'Not now.' }],
      [`${inbox}/IN-1/decisions`, 'POST', { decision: 'restored' }],
    ]);
    expect((fetch.mock.calls[0][1]!.headers as Record<string, string>)['X-Atm-Session']).toBe(TOKEN);
  });

  it('a refusal says why; without the page’s token nothing is sent', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response(JSON.stringify({ error: 'no such item' }), { status: 404 })),
    );
    expect(await editItem('p1', 'IN-9', 'x')).toEqual({ ok: false, message: 'no such item' });
    withToken(null);
    const fetch = vi.fn();
    vi.stubGlobal('fetch', fetch);
    expect(await addItem('p1', 'x')).toEqual({
      ok: false,
      message: 'This page wasn’t served by atm, so it can’t change the inbox.',
    });
    // Only the dev server asked for its token (a test runs as development does): no change sent
    expect(fetch.mock.calls.map(([url]) => url)).toEqual(['/__dev/atm-session']);
  });

  it('in the app, each is the app’s command, with the workspace and project', async () => {
    where.app = true;
    const calls: [string, unknown][] = [];
    mockIPC((command, args) => {
      calls.push([command, args]);
      return { item: { id: 'IN-1' } };
    });
    setApiWorkspace('w');
    await addItem('p1', 'A note');
    await editItem('p1', 'IN-1', 'Better');
    await discardItem('p1', 'IN-1', 'Not now.');
    await restoreItem('p1', 'IN-1');
    const at = { workspace: 'w', project: 'p1' };
    expect(calls).toEqual([
      ['add_inbox_item', { ...at, item: { text: 'A note' } }],
      ['edit_inbox_item', { ...at, id: 'IN-1', edit: { text: 'Better' } }],
      ['decide_inbox_item', { ...at, id: 'IN-1', decision: { decision: 'discarded', reason: 'Not now.' } }],
      ['decide_inbox_item', { ...at, id: 'IN-1', decision: { decision: 'restored' } }],
    ]);
  });
});
