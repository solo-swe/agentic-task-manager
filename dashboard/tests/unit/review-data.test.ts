import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Fetch } from '../../src/lib/api/http';
import { writeStored } from '../../src/lib/prefs';
import {
  SOURCES_CAP,
  VIEWED_CAP,
  VIEWED_KEY,
  isViewed,
  loadReview,
  loadReviews,
  revisionDocuments,
  toggleViewed,
  viewedKeys,
} from '../../src/lib/review-data';
import { viewedKey } from '../../src/lib/reviews/page';
import { NO_QUERY, type ViewFile } from '../../src/lib/reviews/types';

// The task manager's side of the review tool (lib/review-data.ts, ATM-574): its reads through a Fetch it is given (here
// a stub), a round's sides, and the viewed ticks in this browser's storage

function fetching(status: number, body: string) {
  return vi.fn<(url: string) => Promise<Response>>(async () => new Response(body, { status })) as unknown as Fetch &
    ReturnType<typeof vi.fn>;
}

const urlOf = (fetch: ReturnType<typeof fetching>) => (fetch.mock.calls[0] as [string])[0];

const file = (path: string, patch = '@@ -1 +1 @@\n-a\n+b\n'): ViewFile => ({
  path,
  old_path: path,
  status: 'modified',
  binary: false,
  patch,
  before: 'b',
  after: 'a',
});

beforeEach(() => localStorage.clear());
afterEach(() => vi.restoreAllMocks());

describe('the reads', () => {
  it('the list and a page are the server’s JSON, from their routes', async () => {
    const list = fetching(200, JSON.stringify({ reviews: [], you: 'Ana' }));
    expect(await loadReviews('p1', { ...NO_QUERY, status: ['open'] }, list)).toEqual({ reviews: [], you: 'Ana' });
    expect(urlOf(list)).toBe('/api/v1/projects/p1/reviews?status=open');
    const page = fetching(200, JSON.stringify({ review: { reference: 'CR-7' } }));
    expect(await loadReview('p1', 'CR-7', 2, 1, page)).toEqual({ review: { reference: 'CR-7' } });
    expect(urlOf(page)).toBe('/api/v1/projects/p1/reviews/CR-7?round=2&against=1');
  });

  it('a refusal is the server’s message; without one, its status; a failed request, its error', async () => {
    await expect(loadReview('p1', 'CR-9', null, null, fetching(404, '{"error":"no review CR-9"}'))).rejects.toThrow(
      'no review CR-9',
    );
    await expect(loadReviews('p1', NO_QUERY, fetching(500, 'oops'))).rejects.toThrow('HTTP 500');
    await expect(loadReviews('p1', NO_QUERY, fetching(400, '{"error":7}'))).rejects.toThrow('HTTP 400');
    const failing = vi.fn(async () => {
      throw new Error('network down');
    }) as unknown as Fetch;
    await expect(loadReviews('p1', NO_QUERY, failing)).rejects.toThrow('network down');
  });
});

describe('a round’s side', () => {
  it('reads each file at its revision, never changing, with no images and no repository link', async () => {
    const doc = { path: 'a.md', kind: 'markdown', text: '# A' };
    const fetch = fetching(200, JSON.stringify(doc));
    const side = revisionDocuments('p1', '3fa1', fetch);
    expect(side.id).toBe('p1@3fa1');
    expect(side.id).not.toBe(revisionDocuments('p1', '3fa2', fetch).id);
    expect(await side.load('a.md')).toEqual({ unchanged: false, document: doc, validators: {} });
    expect(urlOf(fetch)).toBe('/api/v1/projects/p1/documents?path=a.md&at=3fa1');
    expect(side.imageUrl('img.png')).toBeNull();
    expect(side.repositoryLink(doc as never, { path: 'a.md', lines: null } as never)).toBeNull();
  });

  it('is the same side each time it is asked for, each file read once: a refresh reads nothing again', async () => {
    const fetch = fetching(200, JSON.stringify({ path: 'a.md', kind: 'markdown', text: '# A' }));
    const side = revisionDocuments('p1', 'c1', fetch);
    expect(revisionDocuments('p1', 'c1', fetch)).toBe(side);
    // Another revision, another project or another client is another side
    expect(revisionDocuments('p1', 'c2', fetch)).not.toBe(side);
    expect(revisionDocuments('p2', 'c1', fetch)).not.toBe(side);
    expect(revisionDocuments('p1', 'c1', fetching(200, '{}'))).not.toBe(side);
    const [first, again] = await Promise.all([side.load('a.md'), side.load('a.md')]);
    expect(again).toEqual(first);
    await side.load('a.md');
    await side.load('b.md');
    expect(fetch.mock.calls.map(([url]) => url)).toEqual([
      '/api/v1/projects/p1/documents?path=a.md&at=c1',
      '/api/v1/projects/p1/documents?path=b.md&at=c1',
    ]);
  });

  it('a file that couldn’t be read, refused or failed, is asked for again the next time', async () => {
    // Refused, then a failed request, then read: read once it is, and kept
    const answers: (Response | Error)[] = [
      new Response('{"error":"busy"}', { status: 500 }),
      new Error('network down'),
      new Response('{"path":"a.md"}'),
    ];
    const flaky = vi.fn(async () => {
      const next = answers.shift()!;
      if (next instanceof Error) throw next;
      return next;
    }) as unknown as Fetch & ReturnType<typeof vi.fn>;
    const side = revisionDocuments('p1', 'flaky', flaky);
    expect(await side.load('a.md')).toMatchObject({ error: expect.any(String) });
    await side.load('a.md').then(
      (result) => expect(result).toMatchObject({ error: expect.any(String) }),
      (e: unknown) => expect(e).toBeInstanceOf(Error),
    );
    expect(await side.load('a.md')).toMatchObject({ document: { path: 'a.md' } });
    expect(await side.load('a.md')).toMatchObject({ document: { path: 'a.md' } });
    expect(flaky).toHaveBeenCalledTimes(3);
  });

  it('keeps the latest sides only, per client: the oldest made again when asked for', () => {
    const fetch = fetching(200, '{}');
    const first = revisionDocuments('p1', 'r0', fetch);
    for (let i = 1; i <= SOURCES_CAP; i++) revisionDocuments('p1', `r${i}`, fetch);
    expect(revisionDocuments('p1', `r${SOURCES_CAP}`, fetch)).toBe(revisionDocuments('p1', `r${SOURCES_CAP}`, fetch));
    expect(revisionDocuments('p1', 'r0', fetch)).not.toBe(first);
  });

  it('a file the revision doesn’t have, or one atm won’t show, is the viewer’s to say', async () => {
    expect(await revisionDocuments('p1', 't', fetching(404, '{"error":"a.md: not a file in t"}')).load('a.md')).toEqual(
      {
        unchanged: false,
        error: 'missing',
        message: 'a.md: not a file in t',
      },
    );
    for (const status of [400, 403, 413])
      expect(
        (await revisionDocuments('p1', 't', fetching(status, '{"error":"no"}')).load('a.md')) as object,
      ).toMatchObject({ error: 'refused' });
  });
});

describe('the viewed ticks', () => {
  it('a file is ticked and unticked, by review, view and change, kept in this browser', () => {
    const a = file('a.md');
    expect(isViewed(viewedKeys(), 'r1', 2, null, a)).toBe(false);
    const ticked = toggleViewed('r1', 2, null, a);
    expect(isViewed(ticked, 'r1', 2, null, a)).toBe(true);
    expect(viewedKeys()).toEqual([viewedKey('r1', 2, null, a)]);
    // Another view, review or change of the file isn't ticked by it
    expect(isViewed(ticked, 'r1', 2, 1, a)).toBe(false);
    expect(isViewed(ticked, 'r2', 2, null, a)).toBe(false);
    expect(isViewed(ticked, 'r1', 2, null, file('a.md', '@@ -1 +1 @@\n-a\n+c\n'))).toBe(false);
    expect(isViewed(toggleViewed('r1', 2, null, a), 'r1', 2, null, a)).toBe(false);
    expect(viewedKeys()).toEqual([]);
  });

  it('keeps the latest ticks only, the oldest dropped first', () => {
    writeStored(VIEWED_KEY, { keys: Array.from({ length: VIEWED_CAP }, (_, i) => `k${i}`) });
    const keys = toggleViewed('r1', 1, null, file('new.md'));
    expect(keys).toHaveLength(VIEWED_CAP);
    expect(keys[0]).toBe('k1');
    expect(keys.at(-1)).toBe(viewedKey('r1', 1, null, file('new.md')));
  });

  it('what isn’t a list of keys in storage is no tick', () => {
    for (const stored of [{ keys: 'x' }, { keys: [1, null, 'k'] }, {}]) {
      writeStored(VIEWED_KEY, stored);
      expect(viewedKeys()).toEqual(Array.isArray(stored.keys) ? ['k'] : []);
    }
    localStorage.setItem(VIEWED_KEY, '{not json');
    expect(viewedKeys()).toEqual([]);
  });
});
