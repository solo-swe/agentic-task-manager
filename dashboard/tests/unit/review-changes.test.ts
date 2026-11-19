import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  addComment,
  addDraft,
  closeReview,
  listDrafts,
  openReview,
  pokeReview,
  removeDraft,
  replyTo,
  resolveComment,
  sessionToken,
  submitDecision,
  updateDraft,
} from '../../src/lib/api/review-changes';
import { setApiWorkspace } from '../../src/lib/api/workspace';

// A review's changes from the page (lib/api/review-changes.ts, ATM-574): the REST routes with the page's session token
// in the browser, the app's commands in the app. The transport is stubbed: fetch, the IPC.
const where = vi.hoisted(() => ({ app: false }));
vi.mock('../../src/lib/api/http', async (actual) => ({
  ...(await actual()),
  get inApp() {
    return where.app;
  },
}));

const TOKEN = 'a'.repeat(64);
const reviews = '/api/v2/workspaces/default/projects/p1/reviews';

function withToken(token: string | null) {
  document.head.querySelector('meta[name="atm-session"]')?.remove();
  if (token === null) return;
  const meta = document.createElement('meta');
  meta.name = 'atm-session';
  meta.content = token;
  document.head.prepend(meta);
}

/** fetch answering `status` with `body`, recording what it was asked */
function answering(status: number, body: unknown) {
  const fetch = vi.fn<(url: string, init?: RequestInit) => Promise<Response>>(
    async () => new Response(JSON.stringify(body), { status }),
  );
  vi.stubGlobal('fetch', fetch);
  return fetch;
}

const asked = (fetch: ReturnType<typeof answering>, call = 0) => {
  const [url, init] = fetch.mock.calls[call] as [string, RequestInit];
  return { url, method: init.method, headers: init.headers, body: JSON.parse(String(init.body)) };
};

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

describe('in the browser', () => {
  it('each change is its route, with the page’s token, as JSON', async () => {
    const cases = [
      [
        () =>
          addComment('p1', 'CR-7', { path: 'a.md', lines: [1, 2], side: 'new', quote: 'q', round: 2, body: 'Why?' }),
        'POST',
        `${reviews}/CR-7/comments`,
        { path: 'a.md', lines: [1, 2], side: 'new', quote: 'q', round: 2, body: 'Why?' },
      ],
      [
        () => replyTo('p1', 'CR-7', 'c4', 'Because.'),
        'POST',
        `${reviews}/CR-7/comments/c4/replies`,
        { body: 'Because.' },
      ],
      // ATM-611: a poke, with a message or without one (a blank one is none)
      [() => pokeReview('p1', 'CR-7', 'Where are we?'), 'POST', `${reviews}/CR-7/pokes`, { body: 'Where are we?' }],
      [() => pokeReview('p1', 'CR-7', '  '), 'POST', `${reviews}/CR-7/pokes`, {}],
      // ATM-612: a draft comment, a draft reply, a draft's text, a draft removed (with an empty body)
      [
        () => addDraft('p1', 'CR-7', { path: 'a.md', lines: [1, 1], side: 'new', round: 2, body: 'Why?' }),
        'POST',
        `${reviews}/CR-7/drafts`,
        { path: 'a.md', lines: [1, 1], side: 'new', round: 2, body: 'Why?' },
      ],
      [
        () => addDraft('p1', 'CR-7', { reply_to: 'c4', body: 'No.' }),
        'POST',
        `${reviews}/CR-7/drafts`,
        { reply_to: 'c4', body: 'No.' },
      ],
      [() => updateDraft('p1', 'CR-7', 'd2', 'Why not?'), 'PATCH', `${reviews}/CR-7/drafts/d2`, { body: 'Why not?' }],
      [() => removeDraft('p1', 'CR-7', 'd2'), 'DELETE', `${reviews}/CR-7/drafts/d2`, {}],
      [() => resolveComment('p1', 'CR-7', 'c4', true), 'PATCH', `${reviews}/CR-7/comments/c4`, { resolved: true }],
      [() => resolveComment('p1', 'CR-7', 'c4', false), 'PATCH', `${reviews}/CR-7/comments/c4`, { resolved: false }],
      [
        () => submitDecision('p1', 'CR-7', { decision: 'request_changes', body: 'Two things.' }),
        'POST',
        `${reviews}/CR-7/decisions`,
        { decision: 'request_changes', body: 'Two things.' },
      ],
      [
        () => submitDecision('p1', 'CR-7', { decision: 'approve' }),
        'POST',
        `${reviews}/CR-7/decisions`,
        { decision: 'approve' },
      ],
      // ATM-576: a live review opened, and a review closed
      [
        () => openReview('p1', { title: 'Live', tasks: ['ATM-1'], start: 'live', change: { base: 'main' } }),
        'POST',
        reviews,
        { title: 'Live', tasks: ['ATM-1'], start: 'live', change: { base: 'main' } },
      ],
      [
        () => closeReview('p1', 'CR-7', 'Wrong turn.'),
        'PATCH',
        `${reviews}/CR-7`,
        { status: 'closed', reason: 'Wrong turn.' },
      ],
    ] as const;
    for (const [change, method, url, body] of cases) {
      const fetch = answering(201, { review: 'CR-7' });
      expect(await change()).toEqual({ ok: true, body: { review: 'CR-7' } });
      expect(asked(fetch)).toEqual({
        url,
        method,
        headers: { 'Content-Type': 'application/json', 'X-Atm-Session': TOKEN },
        body,
      });
    }
  });

  it('names the workspace, project, review and comment as the path has them, escaped', async () => {
    setApiWorkspace('my work');
    const fetch = answering(201, {});
    await replyTo('p/1', 'CR 8', 'c 1', 'x');
    expect(asked(fetch).url).toBe('/api/v2/workspaces/my%20work/projects/p%2F1/reviews/CR%208/comments/c%201/replies');
  });

  it('a refusal is its message; a body that isn’t JSON, its status; a failed request, its error', async () => {
    answering(403, { error: "the review's author can't decide it" });
    expect(await submitDecision('p1', 'CR-7', { decision: 'approve' })).toEqual({
      ok: false,
      message: "the review's author can't decide it",
    });
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response('<html>', { status: 502 })),
    );
    expect(await replyTo('p1', 'CR-7', 'c1', 'x')).toEqual({ ok: false, message: 'HTTP 502' });
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => {
        throw new Error('network down');
      }),
    );
    expect(await replyTo('p1', 'CR-7', 'c1', 'x')).toEqual({ ok: false, message: 'network down' });
  });

  it('a refusal whose JSON names no error is its status; a success that isn’t JSON, an empty body', async () => {
    answering(409, { review: 'CR-7' });
    expect(await resolveComment('p1', 'CR-7', 'c1', true)).toEqual({ ok: false, message: 'HTTP 409' });
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response(null, { status: 204 })),
    );
    expect(await resolveComment('p1', 'CR-7', 'c1', true)).toEqual({ ok: true, body: {} });
  });

  it('the drafts are read with a GET, no token needed; a refusal or a failed request is its message', async () => {
    const drafts = [{ id: 'd1', at: 't', body: 'x', kind: 'reply', to: 'c1' }];
    const fetch = answering(200, { review: 'CR-7', drafts });
    withToken(null);
    expect(await listDrafts('p1', 'CR-7')).toEqual({ ok: true, drafts });
    expect(fetch.mock.calls[0][0]).toBe(`${reviews}/CR-7/drafts`);
    expect(fetch.mock.calls[0][1]?.method).toBeUndefined();
    answering(400, { error: 'a person drafts a review’s comments' });
    expect(await listDrafts('p1', 'CR-7')).toEqual({ ok: false, message: 'a person drafts a review’s comments' });
    answering(200, {});
    expect(await listDrafts('p1', 'CR-7')).toEqual({ ok: true, drafts: [] });
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => {
        throw new Error('network down');
      }),
    );
    expect(await listDrafts('p1', 'CR-7')).toEqual({ ok: false, message: 'network down' });
  });

  it('the token is the page’s meta tag; none, none', () => {
    expect(sessionToken()).toBe(TOKEN);
    withToken('');
    expect(sessionToken()).toBeNull();
    withToken(null);
    expect(sessionToken()).toBeNull();
  });

  it('without a token in the page (the dev server’s page), the dev server’s copy; with none there, nothing is sent', async () => {
    withToken(null);
    const fetch = vi.fn<(url: string, init?: RequestInit) => Promise<Response>>(async (url) =>
      url === '/__dev/atm-session'
        ? new Response(JSON.stringify({ token: 'd'.repeat(64) }))
        : new Response(JSON.stringify({ ok: 1 }), { status: 201 }),
    );
    vi.stubGlobal('fetch', fetch);
    expect((await replyTo('p1', 'CR-7', 'c1', 'x')).ok).toBe(true);
    expect(fetch.mock.calls[1][1]?.headers).toMatchObject({ 'X-Atm-Session': 'd'.repeat(64) });

    for (const devAnswer of [
      () => new Response(JSON.stringify({ token: null })),
      () => new Response(JSON.stringify({})),
      () => new Response('not json'),
      () => {
        throw new Error('no dev server');
      },
    ]) {
      const none = vi.fn(async () => devAnswer());
      vi.stubGlobal('fetch', none);
      expect(await replyTo('p1', 'CR-7', 'c1', 'x')).toEqual({
        ok: false,
        message: 'This page wasn’t served by atm, so it can’t change a review.',
      });
      expect(none).toHaveBeenCalledTimes(1);
    }
  });
});

describe('in the app', () => {
  beforeEach(() => {
    where.app = true;
  });

  it('each change is the app’s command of the same name, with the workspace and project', async () => {
    const calls: [string, unknown][] = [];
    mockIPC((command, args) => {
      calls.push([command, args]);
      return { review: 'CR-7' };
    });
    setApiWorkspace('w');
    const fetch = answering(500, {});
    await addComment('p1', 'CR-7', { body: 'On the review.' });
    await replyTo('p1', 'CR-7', 'c4', 'Because.');
    await resolveComment('p1', 'CR-7', 'c4', true);
    await submitDecision('p1', 'CR-7', { decision: 'comment', body: 'Fine.' });
    await openReview('p1', { title: 'Live', start: 'live', change: {} });
    await closeReview('p1', 'CR-7', 'Done elsewhere.');
    await pokeReview('p1', 'CR-7', 'Hello?');
    await pokeReview('p1', 'CR-7');
    await addDraft('p1', 'CR-7', { reply_to: 'c4', body: 'No.' });
    await updateDraft('p1', 'CR-7', 'd1', 'Yes.');
    await removeDraft('p1', 'CR-7', 'd1');
    await listDrafts('p1', 'CR-7');
    const at = { workspace: 'w', project: 'p1', review: 'CR-7' };
    expect(calls).toEqual([
      ['add_review_comment', { ...at, comment: { body: 'On the review.' } }],
      ['reply_to_review_comment', { ...at, comment: 'c4', body: 'Because.' }],
      ['update_review_comment', { ...at, comment: 'c4', update: { resolved: true } }],
      ['submit_review_decision', { ...at, decision: { decision: 'comment', body: 'Fine.' } }],
      ['create_review', { workspace: 'w', project: 'p1', opening: { title: 'Live', start: 'live', change: {} } }],
      ['close_review', { ...at, closing: { status: 'closed', reason: 'Done elsewhere.' } }],
      ['poke_review', { ...at, body: 'Hello?' }],
      ['poke_review', { ...at, body: null }],
      ['add_review_draft', { ...at, draft: { reply_to: 'c4', body: 'No.' } }],
      ['update_review_draft', { ...at, draft: 'd1', body: 'Yes.' }],
      ['remove_review_draft', { ...at, draft: 'd1' }],
      ['list_review_drafts', at],
    ]);
    expect(fetch).not.toHaveBeenCalled();
  });

  it('a refusal is the app’s message, and needs no token', async () => {
    withToken(null);
    mockIPC(() => {
      throw { kind: 'refused', message: 'not yours to resolve' };
    });
    expect(await resolveComment('p1', 'CR-7', 'c1', true)).toEqual({ ok: false, message: 'not yours to resolve' });
    mockIPC(() => {
      throw 'plain';
    });
    expect(await resolveComment('p1', 'CR-7', 'c1', true)).toEqual({ ok: false, message: 'plain' });
  });
});
