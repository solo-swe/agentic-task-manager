import { afterEach, describe, expect, it, vi } from 'vitest';
import { loadDocuments } from '../../src/lib/api/documents';
import { documentsUrl, type Fetch } from '../../src/lib/api/http';
import { callFor } from '../../src/lib/api/ipc';
import { setApiWorkspace } from '../../src/lib/api/workspace';

// The documents' listing client (lib/api/documents.ts, ATM-766), its route and the app's command for it. The
// transport is stubbed: fetch.

afterEach(() => setApiWorkspace('default'));

const answering = (body: unknown, status = 200) =>
  vi.fn(async () => new Response(JSON.stringify(body), { status, headers: { ETag: '"d1"' } })) as unknown as Fetch;

describe('the listing’s route', () => {
  it('every document, or those a search names (trimmed, encoded); another workspace’s v2 route', () => {
    expect(documentsUrl('p1')).toBe('/api/v1/projects/p1/documents');
    expect(documentsUrl('p1', '  ')).toBe('/api/v1/projects/p1/documents');
    expect(documentsUrl('p1', ' the design & more ')).toBe('/api/v1/projects/p1/documents?q=the%20design%20%26%20more');
    setApiWorkspace('w');
    expect(documentsUrl('p1', 'x')).toBe('/api/v2/workspaces/w/projects/p1/documents?q=x');
  });

  it('in the app, the project_documents command with the search', () => {
    expect(callFor(documentsUrl('p1', 'adr'))).toEqual({
      command: 'project_documents',
      args: { workspace: 'default', project: 'p1', q: 'adr' },
    });
  });
});

describe('loadDocuments', () => {
  it('the documents, submodules and how many more, with its validators; anything unexpected left out', async () => {
    const fetch = answering({
      documents: [
        { path: 'README.md', title: 'Read me', bytes: 12, modified_at: '2026-11-24T10:00:00Z', tracked: true },
        { path: 'docs/new.md', title: '', bytes: 'x', tracked: false },
        { title: 'no path' },
        null,
      ],
      submodules: [{ path: 'vendor/lib', checked_out: false }, { checked_out: true }],
      more: 3,
    });
    const result = await loadDocuments('p1', 'read', fetch);
    expect((fetch as unknown as { mock: { calls: unknown[][] } }).mock.calls[0][0]).toBe(
      '/api/v1/projects/p1/documents?q=read',
    );
    expect(result).toEqual({
      unchanged: false,
      listing: {
        documents: [
          { path: 'README.md', title: 'Read me', bytes: 12, modified_at: '2026-11-24T10:00:00Z', tracked: true },
          // No title: its file's name
          { path: 'docs/new.md', title: 'new.md', bytes: 0, modified_at: null, tracked: false },
        ],
        submodules: [{ path: 'vendor/lib', checked_out: false }],
        more: 3,
        git: true,
      },
      validators: { etag: '"d1"' },
    });
  });

  it('a project outside git says so; a body of no shape is an empty listing', async () => {
    const none = await loadDocuments('p1', '', answering({ git: false, documents: [], submodules: [], more: 0 }));
    expect(none.unchanged ? null : none.listing.git).toBe(false);
    const odd = await loadDocuments('p1', '', answering({ documents: 'x', more: -2 }));
    expect(odd.unchanged ? null : odd.listing).toEqual({ documents: [], submodules: [], more: 0, git: true });
  });

  it('asked again with its validators: a 304 is unchanged; an error rejects', async () => {
    const fetch = vi.fn(async () => new Response(null, { status: 304 })) as unknown as Fetch;
    expect(await loadDocuments('p1', '', fetch, { etag: '"d1"' })).toEqual({ unchanged: true });
    await expect(loadDocuments('p1', '', answering({ error: 'refused' }, 400))).rejects.toThrow('HTTP 400');
  });
});
