import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  fetchFile,
  fetchProjectFile,
  reviewUrl,
  reviewsUrl,
  v1DocumentAtUrl,
  type Fetch,
  type Validators,
} from '../../src/lib/api/http';
import { apiWorkspace, setApiWorkspace, workspacePrefix } from '../../src/lib/api/workspace';

/** A fetch answering `status` with `body` and `headers`, recording what it was asked */
function answering(status: number, body = '', headers: Record<string, string> = {}) {
  const fetchFn = vi.fn<(url: string, init?: RequestInit) => Promise<Response>>(
    async () => new Response(status === 304 ? null : body, { status, headers }),
  );
  return { fetchFn: fetchFn as unknown as Fetch, calls: fetchFn.mock.calls };
}

// fetchFile (D-5.1): a conditional request past the browser's cache; 304, 404 and the other errors
describe('fetchFile', () => {
  afterEach(() => setApiWorkspace('default'));

  it('asks past the browser’s cache, with the validators given as the conditional headers', async () => {
    const cases: [Validators | undefined, Record<string, string>][] = [
      [undefined, {}],
      [{}, {}],
      [{ etag: '"v1"' }, { 'If-None-Match': '"v1"' }],
      [{ lastModified: 'Wed, 01 Jan 2026 00:00:00 GMT' }, { 'If-Modified-Since': 'Wed, 01 Jan 2026 00:00:00 GMT' }],
      [
        { etag: '"v1"', lastModified: 'Wed, 01 Jan 2026 00:00:00 GMT' },
        { 'If-None-Match': '"v1"', 'If-Modified-Since': 'Wed, 01 Jan 2026 00:00:00 GMT' },
      ],
    ];
    for (const [since, headers] of cases) {
      const { fetchFn, calls } = answering(200, 'x');
      await fetchFile('/a', fetchFn, since);
      expect(calls[0], JSON.stringify(since)).toEqual(['/a', { cache: 'no-store', headers }]);
    }
  });

  it('a 200 is its text, with the validators it came with, and none it didn’t', async () => {
    const both = answering(200, 'body', { ETag: '"v2"', 'Last-Modified': 'Thu, 02 Jan 2026 00:00:00 GMT' });
    expect(await fetchFile('/a', both.fetchFn)).toEqual({
      unchanged: false,
      text: 'body',
      validators: { etag: '"v2"', lastModified: 'Thu, 02 Jan 2026 00:00:00 GMT' },
    });
    const none = answering(200, '');
    expect(await fetchFile('/a', none.fetchFn)).toEqual({ unchanged: false, text: '', validators: {} });
  });

  it('a 304 after asking is unchanged; without having asked, an error', async () => {
    expect(await fetchFile('/a', answering(304).fetchFn, { etag: '"v1"' })).toEqual({ unchanged: true });
    await expect(fetchFile('/a', answering(304).fetchFn)).rejects.toThrow('HTTP 304');
  });

  it('a 404 is `missing` when given (even empty), else an error; any other error is its status', async () => {
    expect(await fetchFile('/a', answering(404).fetchFn, undefined, '[]')).toEqual({
      unchanged: false,
      text: '[]',
      validators: {},
    });
    expect(await fetchFile('/a', answering(404).fetchFn, undefined, '')).toEqual({
      unchanged: false,
      text: '',
      validators: {},
    });
    await expect(fetchFile('/a', answering(404).fetchFn)).rejects.toThrow('HTTP 404');
    for (const status of [400, 403, 413, 500, 503])
      await expect(fetchFile('/a', answering(status).fetchFn, undefined, '[]'), String(status)).rejects.toThrow(
        `HTTP ${status}`,
      );
  });

  it('a failed request rejects as it failed', async () => {
    const fetchFn = (async () => {
      throw new TypeError('network down');
    }) as unknown as Fetch;
    await expect(fetchFile('/a', fetchFn)).rejects.toThrow('network down');
  });

  it('fetchProjectFile asks for the project’s file in the current workspace', async () => {
    const { fetchFn, calls } = answering(200, '{}');
    await fetchProjectFile('a b', 'claims.json', fetchFn, { etag: '"c"' }, '{}');
    expect(calls[0]).toEqual(['/p/a%20b/claims.json', { cache: 'no-store', headers: { 'If-None-Match': '"c"' } }]);
    setApiWorkspace('w');
    await fetchProjectFile('p1', 'worktrees.json', fetchFn);
    expect(calls[1][0]).toBe('/api/v2/workspaces/w/projects/p1/worktrees');
  });
});

// The workspace requests go to (lib/api/workspace.ts, ADR-003 §5)
describe('the workspace requests go to', () => {
  afterEach(() => setApiWorkspace('default'));

  it('is `default`, with no prefix, until another is set; none given is `default` again', () => {
    expect([apiWorkspace(), workspacePrefix()]).toEqual(['default', null]);
    setApiWorkspace('a b/c');
    expect([apiWorkspace(), workspacePrefix()]).toEqual(['a b/c', '/api/v2/workspaces/a%20b%2Fc']);
    setApiWorkspace('');
    expect([apiWorkspace(), workspacePrefix()]).toEqual(['default', null]);
  });
});

// The review routes' addresses (lib/api/http.ts, ATM-574): the current workspace's, `default`'s the v1 ones
describe('the review routes’ addresses', () => {
  afterEach(() => setApiWorkspace('default'));

  it('a project’s reviews: every filter given, status repeated, none empty', () => {
    expect(reviewsUrl('p1')).toBe('/api/v1/projects/p1/reviews');
    expect(reviewsUrl('p1', { q: '', status: [], author: '' })).toBe('/api/v1/projects/p1/reviews');
    expect(
      reviewsUrl('p 1', {
        q: 'two words',
        status: ['open', 'live'],
        author: 'claude:a',
        reviewer: 'Ana',
        task: 'ATM-1',
        branch: 'dev/1',
      }),
    ).toBe(
      '/api/v1/projects/p%201/reviews?q=two+words&status=open&status=live&author=claude%3Aa&reviewer=Ana&task=ATM-1&branch=dev%2F1',
    );
    setApiWorkspace('my work');
    expect(reviewsUrl('p1', { task: 'T-1' })).toBe('/api/v2/workspaces/my%20work/projects/p1/reviews?task=T-1');
  });

  it('a review’s page: its reference, and the round and the round against when given', () => {
    expect(reviewUrl('p1', 'CR-7')).toBe('/api/v1/projects/p1/reviews/CR-7');
    expect(reviewUrl('p1', 'CR-7', 3)).toBe('/api/v1/projects/p1/reviews/CR-7?round=3');
    expect(reviewUrl('p1', 'CR-7', 3, 1)).toBe('/api/v1/projects/p1/reviews/CR-7?round=3&against=1');
    expect(reviewUrl('p1', 'CR-7', null, 1)).toBe('/api/v1/projects/p1/reviews/CR-7?against=1');
    expect(reviewUrl('p1', 'CR 8/2', null, null)).toBe('/api/v1/projects/p1/reviews/CR%208%2F2');
    setApiWorkspace('w');
    expect(reviewUrl('p1', 'CR-7', 2)).toBe('/api/v2/workspaces/w/projects/p1/reviews/CR-7?round=2');
  });

  it('a file at a revision: the documents route with the revision', () => {
    expect(v1DocumentAtUrl('p1', 'docs/a b.md', '3fa1')).toBe(
      '/api/v1/projects/p1/documents?path=docs%2Fa%20b.md&at=3fa1',
    );
    expect(v1DocumentAtUrl('p1', 'a.md', 'HEAD~1')).toBe('/api/v1/projects/p1/documents?path=a.md&at=HEAD~1');
    setApiWorkspace('w');
    expect(v1DocumentAtUrl('p1', 'a.md', 'r&1')).toBe('/api/v2/workspaces/w/projects/p1/documents?path=a.md&at=r%261');
  });
});
