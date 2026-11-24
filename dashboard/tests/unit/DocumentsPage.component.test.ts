import { fireEvent, render, waitFor } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DocumentEntry, DocumentsListing } from '../../src/lib/api/documents';
import DocumentsPage from '../../src/lib/documents-page/DocumentsPage.svelte';
import { getPref, setPref } from '../../src/lib/prefs';
import { offeredReviewSearch } from '../../src/lib/reviews/search.svelte';
import { dashboard } from '../../src/lib/state.svelte';
import standard from '../fixtures/standard.json';
import { loadIntoDashboard } from './components.helpers';

// The Documents page (lib/documents-page/DocumentsPage.svelte, ATM-766): the client (api/documents.ts) is a stub here, as
// are the address, the navigation and where the page runs

const where = vi.hoisted(() => ({ address: '/projects/fixture/documents', app: false }));
const navigate = vi.hoisted(() => vi.fn());
vi.mock('../../src/lib/navigation', async (actual) => ({
  ...(await actual()),
  navigate,
  currentAddress: () => where.address,
}));
vi.mock('../../src/lib/api/http', async (actual) => ({
  ...(await actual()),
  get inApp() {
    return where.app;
  },
}));
const api = vi.hoisted(() => ({ loadDocuments: vi.fn() }));
vi.mock('../../src/lib/api/documents', () => api);
const tabs = vi.hoisted(() => ({ open: vi.fn() }));
vi.mock('../../src/lib/tab-actions', async (actual) => ({
  ...(await actual()),
  tabActions: () => tabs,
}));

const doc = (path: string, title: string, modified_at: string | null, tracked = true): DocumentEntry => ({
  path,
  title,
  bytes: 10,
  modified_at,
  tracked,
});
const DOCS: DocumentEntry[] = [
  doc('README.md', 'Agentic Task Manager', '2026-11-20T10:00:00Z'),
  doc('docs/sidebar.md', 'The sidebar overhaul', '2026-11-24T08:00:00Z'),
  doc('docs/adr/ADR-003.md', 'Layered crates', '2026-11-01T10:00:00Z'),
  doc('docs/draft.md', 'A draft', '2026-11-24T09:00:00Z', false),
];
const listing = (over: Partial<DocumentsListing> = {}): DocumentsListing => ({
  documents: DOCS,
  submodules: [{ path: 'vendor/compmd', checked_out: false }],
  more: 0,
  git: true,
  ...over,
});
const answer = (l: DocumentsListing) => ({ unchanged: false, listing: l, validators: { etag: '"d1"' } });

const at = (address: string) => {
  where.address = address;
  dashboard.address = address;
};
const rows = (c: HTMLElement) =>
  [...c.querySelectorAll<HTMLElement>('.documents-link, .documents-folder')].map(
    (r) => r.dataset.path ?? `[${r.dataset.folder}]`,
  );

describe('DocumentsPage', () => {
  beforeEach(() => {
    localStorage.clear();
    loadIntoDashboard(standard);
    dashboard.projectId = 'fixture';
    at('/projects/fixture/documents');
    where.app = false;
    api.loadDocuments.mockReset();
    api.loadDocuments.mockResolvedValue(answer(listing()));
    navigate.mockClear();
    tabs.open.mockClear();
  });
  afterEach(() => {
    dashboard.projectId = null;
  });

  it('the tree: folders closed by default, then the root’s files; a submodule marked, one not checked out says so', async () => {
    const { container } = render(DocumentsPage);
    await waitFor(() => expect(container.querySelector('#documents-list')).not.toBeNull());
    expect(api.loadDocuments).toHaveBeenCalledWith('fixture', '', undefined, undefined);
    expect(rows(container)).toEqual(['[docs]', '[vendor]', 'README.md']);
    const docs = container.querySelector<HTMLElement>('[data-folder="docs"]')!;
    expect(docs).toHaveAttribute('aria-expanded', 'false');
    expect(docs.querySelector('.documents-count')).toHaveTextContent('3');
    // Each row: its title, file name and how long ago it changed
    const readme = container.querySelector('[data-path="README.md"]')!;
    expect(readme.querySelector('.documents-title')).toHaveTextContent('Agentic Task Manager');
    expect(readme.querySelector('.documents-file')).toHaveTextContent('README.md');
    expect(readme.querySelector('.documents-ago')?.textContent).toMatch(/ago|20 Nov|Nov/);
    // Opening a folder: its folders, then its files; the untracked one is new; kept for the project
    await fireEvent.click(docs);
    expect(rows(container)).toEqual([
      '[docs]',
      '[docs/adr]',
      'docs/draft.md',
      'docs/sidebar.md',
      '[vendor]',
      'README.md',
    ]);
    expect(container.querySelector('[data-path="docs/draft.md"] .documents-new')).toHaveTextContent('new');
    expect(container.querySelector('[data-path="docs/sidebar.md"] .documents-new')).toBeNull();
    expect(getPref('documentFolders', {})).toEqual({ fixture: ['docs'] });
    await fireEvent.click(container.querySelector('[data-folder="vendor"]')!);
    expect(container.querySelector('[data-folder="vendor/compmd"] .documents-submodule')).toHaveTextContent(
      'submodule, not checked out',
    );
    // Closed again
    await fireEvent.click(docs);
    expect(rows(container)).not.toContain('docs/sidebar.md');
  });

  it('the open folders come back from the preferences, per project', async () => {
    setPref('documentFolders', { fixture: ['docs', 'docs/adr'], other: ['vendor'] });
    const { container } = render(DocumentsPage);
    await waitFor(() => expect(container.querySelector('[data-path="docs/adr/ADR-003.md"]')).not.toBeNull());
    expect(container.querySelector('[data-folder="vendor"]')).toHaveAttribute('aria-expanded', 'false');
  });

  it('Recent: every document by last change, newest first, with its path; the view is a step in the address', async () => {
    at('/projects/fixture/documents?list=recent');
    const { container } = render(DocumentsPage);
    await waitFor(() => expect(container.querySelector('#documents-list')).not.toBeNull());
    expect(rows(container)).toEqual(['docs/draft.md', 'docs/sidebar.md', 'README.md', 'docs/adr/ADR-003.md']);
    expect(container.querySelector('[data-path="docs/adr/ADR-003.md"] .documents-file')).toHaveTextContent(
      'docs/adr/ADR-003.md',
    );
    expect(container.querySelector('[data-view="recent"]')).toHaveAttribute('aria-pressed', 'true');
    await fireEvent.click(container.querySelector('[data-view="tree"]')!);
    expect(navigate).toHaveBeenCalledWith('/projects/fixture/documents');
  });

  it('the search is the header line’s: it filters titles and paths in the page, folders open, in the address', async () => {
    const { container } = render(DocumentsPage);
    await waitFor(() => expect(offeredReviewSearch()).not.toBeNull());
    expect(offeredReviewSearch()?.placeholder).toBe('Search documents');
    offeredReviewSearch()!.search('crates');
    expect(navigate).toHaveBeenCalledWith('/projects/fixture/documents?q=crates', { replace: true });
    at('/projects/fixture/documents?q=crates');
    await waitFor(() => expect(rows(container)).toEqual(['[docs]', '[docs/adr]', 'docs/adr/ADR-003.md']));
    // Every folder open while searching, and not to be closed; the submodules aren't shown
    expect(container.querySelector('[data-folder="docs"]')).toBeDisabled();
    // Nothing found
    at('/projects/fixture/documents?q=nothing');
    await waitFor(() => expect(container.querySelector('#documents-none')).toHaveTextContent('“nothing”'));
    // Only the first listing was asked of atm: the search is the page's
    expect(api.loadDocuments).toHaveBeenCalledTimes(1);
  });

  it('a list atm cut at 500: says how many more, and a search asks atm, which searches every document', async () => {
    api.loadDocuments.mockImplementation(async (_p: string, q: string) =>
      answer(q ? listing({ documents: [doc('far/away.md', 'Far away', null)], more: 0 }) : listing({ more: 12 })),
    );
    const { container } = render(DocumentsPage);
    await waitFor(() => expect(container.querySelector('#documents-more')).toHaveTextContent('And 12 more'));
    at('/projects/fixture/documents?q=away');
    await waitFor(() => expect(rows(container)).toEqual(['[far]', 'far/away.md']));
    expect(api.loadDocuments).toHaveBeenCalledWith('fixture', 'away');
    expect(container.querySelector('#documents-more')).toBeNull();
  });

  it('a click opens the document over the page, a step in its history; ⌘-click: a new tab in the app, the browser’s own outside', async () => {
    const { container } = render(DocumentsPage);
    await waitFor(() => expect(container.querySelector('[data-path="README.md"]')).not.toBeNull());
    const link = container.querySelector<HTMLAnchorElement>('[data-path="README.md"]')!;
    expect(link).toHaveAttribute('href', '/projects/fixture/documents?doc=README.md');
    await fireEvent.click(link);
    expect(navigate).toHaveBeenCalledWith('/projects/fixture/documents?doc=README.md');
    // The browser: a ⌘-click is the link's, a tab of the browser's
    navigate.mockClear();
    expect(await fireEvent.click(link, { metaKey: true })).toBe(true);
    expect(navigate).not.toHaveBeenCalled();
    expect(tabs.open).not.toHaveBeenCalled();
    // The app: a tab of its own, in the background
    where.app = true;
    await fireEvent.click(link, { metaKey: true });
    expect(tabs.open).toHaveBeenCalledWith('/projects/fixture/documents?doc=README.md', true);
    expect(navigate).not.toHaveBeenCalled();
  });

  it('none yet, a project outside git, and a listing that can’t be read each say so', async () => {
    api.loadDocuments.mockResolvedValue(answer(listing({ documents: [], submodules: [] })));
    const empty = render(DocumentsPage);
    await waitFor(() => expect(empty.container.querySelector('#documents-empty')).not.toBeNull());
    empty.unmount();
    api.loadDocuments.mockResolvedValue(answer(listing({ documents: [], submodules: [], git: false })));
    const noGit = render(DocumentsPage);
    await waitFor(() => expect(noGit.container.querySelector('#documents-no-git')).not.toBeNull());
    noGit.unmount();
    api.loadDocuments.mockRejectedValue(new Error('HTTP 500'));
    const broken = render(DocumentsPage);
    await waitFor(() => expect(broken.container.querySelector('#documents-problem')).toHaveTextContent('HTTP 500'));
  });

  it('asked again by its validators: unchanged keeps what is shown', async () => {
    vi.useFakeTimers();
    const { container } = render(DocumentsPage);
    await vi.waitFor(() => expect(container.querySelector('#documents-list')).not.toBeNull());
    api.loadDocuments.mockResolvedValue({ unchanged: true });
    await vi.advanceTimersByTimeAsync(10_000);
    vi.useRealTimers();
    expect(api.loadDocuments).toHaveBeenLastCalledWith('fixture', '', undefined, { etag: '"d1"' });
    expect(rows(container)).toEqual(['[docs]', '[vendor]', 'README.md']);
  });
});
