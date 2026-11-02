import { describe, expect, it, vi } from 'vitest';
import {
  DEFAULT_PROJECT_NAME,
  loadArchive,
  loadProjects,
  loadTasks,
  newBuildReady,
  parseTasksFile,
  projectFileUrl,
  searchProjects,
  serverBuild,
} from '../../src/lib/load';
import archive from '../fixtures/archive.json';
import standard from '../fixtures/standard.json';
import noMeta from '../fixtures/no-meta.json';
import { asTaskViews, v1Tasks } from './api.helpers';

const jsonResponse = (body: unknown, status = 200) =>
  new Response(typeof body === 'string' ? body : JSON.stringify(body), { status });

describe('parseTasksFile', () => {
  it('indexes tasks by id and reads the project name', () => {
    const { tasks, taskMap, projectName } = parseTasksFile(standard);
    expect(tasks).toHaveLength(12);
    expect(taskMap.get('T-004')?.name).toBe('User login');
    expect(projectName).toBe('Standard Project');
  });

  it('falls back without meta or tasks', () => {
    expect(parseTasksFile(noMeta).projectName).toBe(DEFAULT_PROJECT_NAME);
    expect(parseTasksFile({ meta: { project: '' } }).projectName).toBe(DEFAULT_PROJECT_NAME);
    expect(parseTasksFile({}).tasks).toEqual([]);
  });

  it('keeps the last task when ids are duplicated', () => {
    const t = { phase: 0, type: 'feature', description: '', status: 'todo' };
    const { taskMap } = parseTasksFile({
      tasks: [
        { ...t, id: 'T-1', name: 'a' },
        { ...t, id: 'T-1', name: 'b' },
      ],
    });
    expect(taskMap.get('T-1')?.name).toBe('b');
  });
});

describe('a new dashboard build on the server', () => {
  it("reads the server's build from __atm/whoami; anything else is unknown", async () => {
    const fetchFn = vi.fn().mockResolvedValue(jsonResponse({ atm: true, build: 'b2' }));
    expect(await serverBuild(fetchFn)).toBe('b2');
    expect(fetchFn).toHaveBeenCalledWith('__atm/whoami', { cache: 'no-store' });
    expect(await serverBuild(vi.fn().mockResolvedValue(jsonResponse('nope', 404)))).toBeNull();
    expect(await serverBuild(vi.fn().mockResolvedValue(jsonResponse({ atm: true })))).toBeNull();
    expect(await serverBuild(vi.fn().mockRejectedValue(new TypeError('offline')))).toBeNull();
  });

  it('is ready only when both builds are known and differ', () => {
    expect(newBuildReady('b1', 'b2')).toBe(true);
    expect(newBuildReady('b1', 'b1')).toBe(false);
    expect(newBuildReady(undefined, 'b2')).toBe(false);
    expect(newBuildReady('b1', null)).toBe(false);
    // The build the user dismissed isn't offered again; another one is
    expect(newBuildReady('b1', 'b2', 'b2')).toBe(false);
    expect(newBuildReady('b1', 'b3', 'b2')).toBe(true);
  });
});

describe('searchProjects (T-261)', () => {
  it('asks api/search past the cache, and keeps only task IDs per project', async () => {
    const fetchFn = vi.fn().mockResolvedValue(jsonResponse({ a1: ['T-1', 2, 'T-3'], b2: 'nope', c3: [] }));
    const found = await searchProjects('a b&c', fetchFn);
    expect(fetchFn).toHaveBeenCalledWith('api/search?q=a%20b%26c', { cache: 'no-store' });
    expect([...found].map(([id, ids]) => [id, [...ids]])).toEqual([
      ['a1', ['T-1', 'T-3']],
      ['c3', []],
    ]);
    expect((await searchProjects('x', vi.fn().mockResolvedValue(jsonResponse(['nope'])))).size).toBe(0);
  });

  it('throws on a failed request', async () => {
    await expect(searchProjects('x', vi.fn().mockResolvedValue(jsonResponse('no', 500)))).rejects.toThrow(
      'api/search: HTTP 500',
    );
  });
});

describe('projectFileUrl', () => {
  it("is the file under the project's id, relative to the page", () => {
    expect(projectFileUrl('abcd1234', 'tasks.json')).toBe('p/abcd1234/tasks.json');
    expect(projectFileUrl('a b/c', 'claims.json')).toBe('p/a%20b%2Fc/claims.json');
  });
});

describe('loadProjects', () => {
  const summary = {
    total: 3,
    done: 1,
    skipped: 0,
    percent: 33,
    agents_working: 0,
    needs_you: { review: 0, blocked_on_user: 0 },
    last_activity: null,
    completions_14d: [],
  };

  it('fetches api/projects past the browser cache, conditionally once it has validators', async () => {
    const body = { projects: [{ id: 'abcd1234', name: 'Demo', path: '/x', missing: false, summary }] };
    const fetchFn = vi.fn().mockResolvedValue(new Response(JSON.stringify(body), { headers: { ETag: '"p1"' } }));
    const result = await loadProjects(fetchFn);
    expect(fetchFn).toHaveBeenCalledWith('api/projects', { cache: 'no-store', headers: {} });
    expect(result).toEqual({
      unchanged: false,
      projects: [{ ...body.projects[0], worktrees: [], logo: null }],
      validators: { etag: '"p1"' },
    });
    const again = vi.fn().mockResolvedValue(new Response(null, { status: 304 }));
    expect(await loadProjects(again, { etag: '"p1"' })).toEqual({ unchanged: true });
    expect(again).toHaveBeenCalledWith('api/projects', { cache: 'no-store', headers: { 'If-None-Match': '"p1"' } });
  });

  it('leaves out entries without an id and fills in what is missing', async () => {
    const body = { projects: [{ name: 'no id' }, { id: '' }, { id: 'x1', missing: 'yes', summary: 'odd' }, null] };
    const result = await loadProjects(vi.fn().mockResolvedValue(jsonResponse(body)));
    expect(!result.unchanged && result.projects).toEqual([
      { id: 'x1', name: 'x1', path: '', missing: false, summary: null, worktrees: [], logo: null },
    ]);
    const empty = await loadProjects(vi.fn().mockResolvedValue(jsonResponse({})));
    expect(!empty.unchanged && empty.projects).toEqual([]);
  });

  it("lists each project's git worktrees, main first, leaving out malformed ones", async () => {
    const worktrees = [
      { path: '/x', branch: 'main', main: true },
      { path: '/x-2', branch: 7, main: 'no' },
      { branch: 'no-path' },
      null,
    ];
    const result = await loadProjects(vi.fn().mockResolvedValue(jsonResponse({ projects: [{ id: 'x1', worktrees }] })));
    expect(!result.unchanged && result.projects[0].worktrees).toEqual([
      { path: '/x', branch: 'main', main: true },
      { path: '/x-2', branch: null, main: false },
    ]);
  });

  it("takes a project's logo only as the server's own p/<id>/logo route", async () => {
    const logos = ['p/x1/logo', 'https://example.com/logo.png', '//evil/logo', 'p/x1/../../logo', 'javascript:x', 7];
    const body = { projects: logos.map((logo, i) => ({ id: `x${i + 1}`, logo })) };
    const result = await loadProjects(vi.fn().mockResolvedValue(jsonResponse(body)));
    expect(!result.unchanged && result.projects.map((p) => p.logo)).toEqual([
      'p/x1/logo',
      null,
      null,
      null,
      null,
      null,
    ]);
  });

  it('rejects with the HTTP status', async () => {
    await expect(loadProjects(vi.fn().mockResolvedValue(jsonResponse('nope', 404)))).rejects.toThrow('HTTP 404');
  });
});

describe('loadTasks', () => {
  it("fetches the project's tasks from the v1 route past the browser cache, and keeps its validators", async () => {
    const response = new Response(JSON.stringify(asTaskViews(standard)), {
      headers: { ETag: '"abc"', 'Last-Modified': 'Sat, 24 Oct 2026 10:00:00 GMT' },
    });
    const fetchFn = vi.fn().mockResolvedValue(response);
    const result = await loadTasks('abcd1234', fetchFn);
    expect(fetchFn).toHaveBeenCalledWith(v1Tasks('abcd1234'), { cache: 'no-store', headers: {} });
    expect(result.unchanged).toBe(false);
    if (result.unchanged) return;
    expect(result.loaded.tasks).toHaveLength(12);
    // The task as stored, with what atm derives for it beside its own fields (T-366)
    expect(result.loaded.tasks[0]).toEqual({ ...standard.tasks[0], derived: {} });
    expect(result.loaded.projectName).toBe('Standard Project'); // its meta, in the same snapshot
    expect(result.validators).toEqual({ etag: '"abc"', lastModified: 'Sat, 24 Oct 2026 10:00:00 GMT' });
  });

  it('asks whether it changed; a 304 means unchanged, without parsing', async () => {
    const fetchFn = vi.fn().mockResolvedValue(new Response(null, { status: 304 }));
    const since = { etag: '"abc"', lastModified: 'Sat, 24 Oct 2026 10:00:00 GMT' };
    expect(await loadTasks('abcd1234', fetchFn, since)).toEqual({ unchanged: true });
    expect(fetchFn).toHaveBeenCalledWith(v1Tasks('abcd1234'), {
      cache: 'no-store',
      headers: { 'If-None-Match': '"abc"', 'If-Modified-Since': 'Sat, 24 Oct 2026 10:00:00 GMT' },
    });
  });

  it('takes the task data’s size from the server, for the archive notice', async () => {
    const result = await loadTasks(
      'p1',
      vi.fn().mockResolvedValue(new Response(JSON.stringify(asTaskViews(standard, 4321)))),
    );
    expect(!result.unchanged && result.loaded.bytes).toBe(4321);
  });

  it('a 304 without having asked is an error', async () => {
    const fetchFn = vi.fn().mockResolvedValue(new Response(null, { status: 304 }));
    await expect(loadTasks('p1', fetchFn)).rejects.toThrow('HTTP 304');
  });

  it('rejects with the HTTP status', async () => {
    const fetchFn = vi.fn().mockResolvedValue(jsonResponse('not found', 404));
    await expect(loadTasks('p1', fetchFn)).rejects.toThrow('HTTP 404');
  });

  it('rejects on invalid JSON', async () => {
    const fetchFn = vi.fn().mockResolvedValue(jsonResponse('{ not json'));
    await expect(loadTasks('p1', fetchFn)).rejects.toThrow(SyntaxError);
  });
});

describe('loadArchive', () => {
  it("fetches the project's tasks-archive.json, conditionally once it has validators", async () => {
    const fetchFn = vi.fn().mockResolvedValue(new Response(JSON.stringify(archive), { headers: { ETag: '"a1"' } }));
    const result = await loadArchive('p1', fetchFn);
    expect(fetchFn).toHaveBeenCalledWith('p/p1/tasks-archive.json', { cache: 'no-store', headers: {} });
    expect(!result.unchanged && result.tasks.map((t) => t.id)).toEqual(['T-090', 'T-091', 'T-092']);
    expect(!result.unchanged && result.validators).toEqual({ etag: '"a1"' });
    const again = vi.fn().mockResolvedValue(new Response(null, { status: 304 }));
    expect(await loadArchive('p1', again, { etag: '"a1"' })).toEqual({ unchanged: true });
  });

  it('a missing archive (404) is an empty one; a malformed one has no tasks; a server error rejects', async () => {
    const missing = await loadArchive('p1', vi.fn().mockResolvedValue(jsonResponse('not found', 404)));
    expect(missing).toEqual({ unchanged: false, tasks: [], validators: {} });
    const odd = await loadArchive('p1', vi.fn().mockResolvedValue(jsonResponse({ tasks: 'x' })));
    expect(!odd.unchanged && odd.tasks).toEqual([]);
    await expect(loadArchive('p1', vi.fn().mockResolvedValue(jsonResponse('boom', 500)))).rejects.toThrow('HTTP 500');
  });
});
