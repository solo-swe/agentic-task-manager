import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DashboardState } from '../../src/lib/state.svelte';
import standard from '../fixtures/standard.json';
import { asTaskViews, v1Tasks } from './api.helpers';

const ok = (body: unknown) => Promise.resolve(new Response(JSON.stringify(body)));
const project = (id: string, missing = false) => ({ id, name: id, path: `/${id}`, missing, summary: null });
const PROJECTS = { projects: [project('p1')] };
/** Plays atm serve: the project list, and the standard fixture as each project's tasks and files */
const server = (projects: unknown = PROJECTS) =>
  vi.fn((url: string) =>
    ok(
      String(url).startsWith('api/projects')
        ? projects
        : String(url).startsWith('api/v1/')
          ? asTaskViews(standard)
          : standard,
    ),
  );

describe('DashboardState', () => {
  let fetchMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    fetchMock = server();
    vi.stubGlobal('fetch', fetchMock);
  });
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it('starts empty and unloaded', () => {
    const s = new DashboardState();
    expect(s.loaded).toBe(false);
    expect(s.tasks).toEqual([]);
    expect(s.projectName).toBe('Task Dashboard');
  });

  it('loads tasks', async () => {
    const s = new DashboardState();
    await s.load();
    expect(s.loaded).toBe(true);
    expect(s.loadError).toBeNull();
    expect(s.tasks).toHaveLength(12);
    expect(s.taskMap.get('T-004')?.name).toBe('User login');
    expect(s.projectName).toBe('Standard Project');
    expect(s.filtered).toHaveLength(12);
  });

  describe('pages', () => {
    afterEach(() => {
      history.replaceState(null, '', '/');
      localStorage.clear();
    });
    // What the page loads (the server's whoami is asked alongside, for the "new version" notice)
    const urls = () =>
      fetchMock.mock.calls.map(([url]) => String(url).split('?')[0]).filter((u) => u !== '__atm/whoami');

    it('with one project, loads the project list, then its tasks (v1) and its files from p/<id>/', async () => {
      fetchMock = server({ projects: [project('gone', true), project('p2')] });
      vi.stubGlobal('fetch', fetchMock);
      const s = new DashboardState();
      await s.load();
      expect(s.route).toEqual({ page: 'project', id: 'p2' });
      expect(s.projectId).toBe('p2');
      expect(urls()).toEqual(['api/projects', v1Tasks('p2'), 'p/p2/claims.json', 'p/p2/worktrees.json']);
    });

    it('with several, lands on the overview without opening a project; it reads every project’s tasks', async () => {
      fetchMock = server({ projects: [project('p2'), project('p3')] });
      vi.stubGlobal('fetch', fetchMock);
      const s = new DashboardState();
      await s.load();
      expect(s.route).toEqual({ page: 'overview' });
      expect([s.loaded, s.loadError, s.projectsLoaded]).toEqual([false, null, true]);
      // For the overview's views across projects (T-333, T-328): each project's tasks, archive (T-355) and claims, no
      // project opened
      expect(urls()[0]).toBe('api/projects');
      expect(urls().slice(1).sort()).toEqual([
        v1Tasks('p2'),
        v1Tasks('p2'), // ?archived=true: the archive (T-372)
        v1Tasks('p3'),
        v1Tasks('p3'),
        'p/p2/claims.json',
        'p/p3/claims.json',
      ]);
      expect(s.projectSources.map((p) => p.project.id)).toEqual(['p2', 'p3']);
    });

    it('#project= picks one; a later hash change switches project and restores that one’s view', async () => {
      fetchMock = server({ projects: [project('p2'), project('p3')] });
      vi.stubGlobal('fetch', fetchMock);
      history.replaceState(null, '', '/#project=p3');
      const s = new DashboardState();
      await s.load();
      expect([s.projectId, s.loaded]).toEqual(['p3', true]);
      s.query = 'login';
      s.saveView();
      s.showDetail('T-004');

      history.replaceState(null, '', '/#project=p2');
      s.onHashChange();
      await vi.waitFor(() => expect(s.projectId).toBe('p2'));
      await vi.waitFor(() => expect(s.loaded).toBe(true));
      expect([s.detailOpen, s.query]).toEqual([false, '']);
      expect(urls()).toContain(v1Tasks('p2'));
    });

    it('a hash change that only switches the tab reloads nothing', async () => {
      const s = new DashboardState();
      await s.load();
      fetchMock.mockClear();
      history.replaceState(null, '', '/#view=kanban');
      s.onHashChange();
      expect(fetchMock).not.toHaveBeenCalled();
    });

    it('without a project to show, says how to add one; an unknown project is its own page', async () => {
      vi.stubGlobal('fetch', server({ projects: [project('gone', true)] }));
      const s = new DashboardState();
      await s.load();
      expect([s.route, s.loadError]).toEqual([{ page: 'none' }, null]);
      history.replaceState(null, '', '/#project=nope');
      await s.load();
      expect(s.route).toEqual({ page: 'unknown', id: 'nope' });
    });
  });

  it('keeps showing the same project across reloads', async () => {
    const s = new DashboardState();
    await s.load();
    fetchMock.mockClear();
    await s.reload();
    expect(fetchMock.mock.calls.map(([url]) => String(url))).toContain(v1Tasks('p1'));
  });

  it('records an initial load failure', async () => {
    fetchMock.mockResolvedValueOnce(new Response('nope', { status: 404 }));
    const s = new DashboardState();
    await s.load();
    expect(s.loaded).toBe(false);
    expect(s.loadError).toBe('HTTP 404');
  });

  it('keeps current data when a reload fails', async () => {
    const error = vi.spyOn(console, 'error').mockImplementation(() => {});
    const s = new DashboardState();
    await s.load();
    fetchMock.mockResolvedValueOnce(new Response('nope', { status: 500 }));
    await s.reload();
    expect(s.tasks).toHaveLength(12);
    expect(s.loadError).toBeNull();
    expect(error).toHaveBeenCalledWith('Reload failed:', expect.any(Error));
  });

  it('a successful reload replaces data and clears a load error', async () => {
    fetchMock.mockResolvedValueOnce(new Response('nope', { status: 404 }));
    const s = new DashboardState();
    await s.load();
    await s.reload();
    expect(s.loaded).toBe(true);
    expect(s.loadError).toBeNull();
    expect(s.tasks).toHaveLength(12);
  });

  it('shows the reloading flag for 400ms', async () => {
    vi.useFakeTimers();
    const s = new DashboardState();
    await s.reload();
    expect(s.reloading).toBe(true);
    vi.advanceTimersByTime(400);
    expect(s.reloading).toBe(false);
  });

  it('filters', async () => {
    const s = new DashboardState();
    await s.load();
    s.toggleFilter('sprint', 'Phase 1');
    expect(s.filtered.map((t) => t.id)).toEqual(['T-004', 'T-005', 'T-006', 'T-007']);
    s.toggleFilter('sprint', 'all');
    expect(s.filtered).toHaveLength(12);
  });

  it('opens and closes the detail panel for known tasks only', async () => {
    const s = new DashboardState();
    await s.load();
    s.showDetail('T-999');
    expect(s.detailOpen).toBe(false);
    s.showDetail('T-004');
    expect([s.detailId, s.detailOpen]).toEqual(['T-004', true]);
    s.closeDetail();
    expect([s.detailId, s.detailOpen]).toEqual(['T-004', false]);
  });
});

describe('DashboardState auto-refresh preference', () => {
  beforeEach(() => localStorage.clear());

  it('is on every 5 seconds by default', () => {
    const s = new DashboardState();
    expect([s.autoRefresh, s.autoRefreshSeconds]).toEqual([true, 5]);
  });

  it('remembers the choice and interval for the next session', () => {
    const first = new DashboardState();
    first.setAutoRefresh(false);
    first.setAutoRefreshSeconds(30);
    const next = new DashboardState();
    expect([next.autoRefresh, next.autoRefreshSeconds]).toEqual([false, 30]);
  });
});

describe('DashboardState update notice', () => {
  let held = 'b2';
  let meta: HTMLMetaElement;
  beforeEach(() => {
    localStorage.clear();
    history.replaceState(null, '', '/');
    held = 'b2';
    // This page is build b1; the server's whoami says which build it holds now
    meta = Object.assign(document.createElement('meta'), { name: 'atm-dashboard-build', content: 'b1' });
    document.head.append(meta);
    const files = server();
    const whoami = () => ok({ atm: true, build: held });
    vi.stubGlobal(
      'fetch',
      vi.fn((url: string) => (url === '__atm/whoami' ? whoami() : files(url))),
    );
  });
  afterEach(() => {
    meta.remove();
    vi.unstubAllGlobals();
    localStorage.clear();
  });

  /** A poll, then the whoami check it starts in the background. */
  async function poll(s: DashboardState) {
    await s.load();
    await new Promise((resolve) => setTimeout(resolve, 0));
  }

  it('dismissing a build keeps it away after a reload and in other tabs; another build brings it back', async () => {
    const s = new DashboardState();
    await poll(s);
    expect(s.newVersionReady).toBe(true);
    s.dismissNewVersion();
    expect(s.newVersionReady).toBe(false);
    await poll(s);
    expect(s.newVersionReady).toBe(false);

    const reloaded = new DashboardState(); // a reload, or another tab
    await poll(reloaded);
    expect(reloaded.newVersionReady).toBe(false);

    held = 'b3';
    await poll(reloaded);
    expect(reloaded.newVersionReady).toBe(true);
  });

  it('dismissed in another tab: this one hides it on its next check', async () => {
    const s = new DashboardState();
    const other = new DashboardState();
    await poll(s);
    await poll(other);
    expect([s.newVersionReady, other.newVersionReady]).toEqual([true, true]);
    other.dismissNewVersion();
    await poll(s);
    expect(s.newVersionReady).toBe(false);
  });

  it('without storage, dismissing still holds for this page', async () => {
    const s = new DashboardState();
    await poll(s);
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('blocked');
    });
    s.dismissNewVersion();
    await poll(s);
    expect(s.newVersionReady).toBe(false);
    vi.restoreAllMocks();
  });

  it('a failed check leaves the notice as it was', async () => {
    const s = new DashboardState();
    await poll(s);
    expect(s.newVersionReady).toBe(true);
    vi.stubGlobal(
      'fetch',
      vi.fn(() => Promise.reject(new TypeError('offline'))),
    );
    await poll(s);
    expect(s.newVersionReady).toBe(true);
  });
});

describe('DashboardState saved view', () => {
  beforeEach(() => {
    localStorage.clear();
    history.replaceState(null, '', '/');
    vi.stubGlobal('fetch', server());
  });
  afterEach(() => vi.unstubAllGlobals());

  async function changeEverything() {
    const s = new DashboardState();
    await s.load();
    s.toggleFilter('type', 'feature');
    s.query = 'login';
    s.selectTab('agents');
    s.kanbanGroupBySprint = true;
    s.kanbanShowSkipped = true;
    s.kanbanShowAllDone = true;
    s.progressSprint = 'Phase 1';
    s.graphFiltered = 'dim';
    s.graphView = { k: 2, x: 10, y: 20 };
    s.agentsWindow = { start: 1000, end: 400_000, followEnd: true };
    s.agentsLive = true;
    s.activityRange = { preset: 'week' };
    s.saveView();
    return s;
  }

  it('brings back every selection on the next visit', async () => {
    const first = await changeEverything();
    history.replaceState(null, '', '/');
    const next = new DashboardState();
    await next.load();
    expect(next.currentView()).toEqual(first.currentView());
    expect(next.tab).toBe('agents');
    expect([...next.filters.type]).toEqual(['feature']);
  });

  it('a tab in the URL wins over the saved one', async () => {
    await changeEverything();
    history.replaceState(null, '', '/#view=kanban');
    const next = new DashboardState();
    await next.load();
    expect(next.tab).toBe('kanban');
    expect(next.query).toBe('login');
  });

  it('reset forgets the saved view and goes back to the defaults', async () => {
    const s = await changeEverything();
    s.resetView();
    expect(s.query).toBe('');
    expect(s.filters.type.size).toBe(0);
    expect(s.tab).toBe('kanban');
    expect([s.kanbanShowAllDone, s.agentsWindow, s.graphView, s.graphFiltered]).toEqual([false, null, null, 'hide']);
    history.replaceState(null, '', '/');
    const next = new DashboardState();
    await next.load();
    expect(next.tab).toBe('kanban');
  });
});

describe('DashboardState appearance', () => {
  beforeEach(() => localStorage.clear());

  it('remembers mode and theme per browser; unknown themes fall back to Default', () => {
    const first = new DashboardState();
    first.setModePref('dark');
    first.setTheme('nord');
    const next = new DashboardState();
    expect([next.modePref, next.mode, next.theme]).toEqual(['dark', 'dark', 'nord']);
    next.setTheme('vaporwave');
    expect(next.theme).toBe('default');
  });
});
