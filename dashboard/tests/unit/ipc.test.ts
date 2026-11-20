import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it } from 'vitest';
import { callFor, ipcFetch } from '../../src/lib/api/ipc';

describe('the IPC transport (T-457)', () => {
  afterEach(() => clearMocks());

  it('maps the v2 routes, and the v1 ones as the default workspace’s, to the app’s commands', () => {
    const at = (workspace: string) => ({ workspace, project: 'p1' });
    expect(callFor('/api/v2/workspaces')).toEqual({ command: 'list_workspaces', args: {} });
    expect(callFor('/api/v2/workspaces/my%20work/projects')).toEqual({
      command: 'list_projects',
      args: { workspace: 'my work' },
    });
    expect(callFor('/api/v2/workspaces/w/search?q=a+b')).toEqual({
      command: 'search',
      args: { workspace: 'w', q: 'a b' },
    });
    expect(callFor('/api/v2/workspaces/w/projects/p1/claims')).toEqual({ command: 'claims', args: at('w') });
    expect(callFor('/api/v2/workspaces/w/projects/p1/logo')).toEqual({ command: 'logo', args: at('w'), image: true });
    expect(callFor('/api/v2/workspaces/w/projects/p1/documents?path=docs/a.md')).toEqual({
      command: 'document',
      args: { ...at('w'), path: 'docs/a.md' },
    });
    expect(callFor('/api/projects')).toEqual({ command: 'list_projects', args: { workspace: 'default' } });
    expect(callFor('/api/search?q=x')).toEqual({ command: 'search', args: { workspace: 'default', q: 'x' } });
    expect(callFor('/p/p1/worktrees.json')).toEqual({ command: 'worktrees', args: at('default') });
    expect(callFor('/api/v1/projects/p1/files?path=x.png')).toEqual({
      command: 'file',
      args: { ...at('default'), path: 'x.png' },
      image: true,
    });
    const tasks = callFor('/api/v1/projects/p1/tasks?archived=true&status=todo&status=blocked&sprint=A%2C%20B')!;
    expect(tasks.command).toBe('project_tasks');
    expect(tasks.args.filters).toMatchObject({
      archived: true,
      status: ['todo', 'blocked'],
      sprint: ['A, B'],
      ready: false,
    });
    // Nothing else: no whoami, no task file, nothing deeper than a route
    for (const url of [
      '/__atm/whoami',
      '/p/p1/tasks.json',
      '/api/v2/workspaces/w/projects/p1/tasks/x',
      '/api/v2',
      '/api/v1/projects/p1/sprints',
      '/api/v2/workspaces/w/settings',
      '/api/v2/workspaces/w/search/x',
      '/api/projects/p1',
    ])
      expect(callFor(url), url).toBeNull();
  });

  it('maps a project’s reviews, a review’s page and a file at a revision to their commands (ATM-574)', () => {
    const at = (workspace: string) => ({ workspace, project: 'p1' });
    const none = { q: '', status: [], author: undefined, reviewer: undefined, task: undefined, branch: undefined };
    for (const [url, workspace] of [
      ['/api/v2/workspaces/w/projects/p1/reviews', 'w'],
      ['/api/v1/projects/p1/reviews', 'default'],
    ] as const)
      expect(callFor(url), url).toEqual({ command: 'project_reviews', args: { ...at(workspace), filters: none } });
    expect(
      callFor(
        '/api/v1/projects/p1/reviews?q=two+words&status=open&status=live&status=&author=claude%3Aa&reviewer=Ana&task=ATM-1&branch=dev%2F1',
      ),
    ).toEqual({
      command: 'project_reviews',
      args: {
        ...at('default'),
        filters: {
          q: 'two words',
          status: ['open', 'live'],
          author: 'claude:a',
          reviewer: 'Ana',
          task: 'ATM-1',
          branch: 'dev/1',
        },
      },
    });
    expect(callFor('/api/v2/workspaces/w/projects/p1/reviews/CR-7?round=3&against=1')).toEqual({
      command: 'project_review',
      args: { ...at('w'), review: 'CR-7', round: 3, against: 1 },
    });
    expect(callFor('/api/v1/projects/p1/reviews/CR%208.2')).toEqual({
      command: 'project_review',
      args: { ...at('default'), review: 'CR 8.2', round: undefined, against: undefined },
    });
    // A round that isn't a number goes to the command as it is, which refuses it as the route does
    expect(callFor('/api/v1/projects/p1/reviews/CR-7?round=x')!.args.round).toBeNaN();
    expect(callFor('/api/v1/projects/p1/documents?path=a.md&at=3fa1')).toEqual({
      command: 'document',
      args: { ...at('default'), path: 'a.md', at: '3fa1' },
    });
    expect(callFor('/api/v1/projects/p1/documents?path=a.md&at=')!.args.at).toBeUndefined();
    // No review named, deeper than a review, or under p/
    for (const url of [
      '/api/v1/projects/p1/reviews/',
      '/api/v1/projects/p1/reviews/CR-7/comments',
      '/api/v2/workspaces/w/projects/p1/reviews/CR-7/x',
      '/p/p1/reviews',
      '/p/p1/tasks',
      '/p/p1/documents?path=a.md',
      '/api/v1/projects/p1/tasks/',
    ])
      expect(callFor(url), url).toBeNull();
  });

  it('answers with the command’s JSON, an image’s bytes, or the error as its HTTP status', async () => {
    mockIPC((command, args) => {
      if (command === 'list_projects')
        return { projects: [{ id: 'p1', workspace: (args as { workspace: string }).workspace }] };
      if (command === 'logo') return { content_type: 'image/png', data: [137, 80, 78, 71] };
      if (command === 'claims') throw { kind: 'not_found', message: 'unknown workspace' };
      return null;
    });
    const projects = await ipcFetch('/api/v2/workspaces/work/projects');
    expect(projects.status).toBe(200);
    expect(await projects.json()).toEqual({ projects: [{ id: 'p1', workspace: 'work' }] });
    const logo = await ipcFetch('/p/p1/logo');
    expect(logo.headers.get('Content-Type')).toBe('image/png');
    expect([...new Uint8Array(await logo.arrayBuffer())]).toEqual([137, 80, 78, 71]);
    const missing = await ipcFetch('/api/v2/workspaces/nope/projects/p1/claims');
    expect([missing.status, await missing.json()]).toEqual([404, { error: 'unknown workspace' }]);
    expect((await ipcFetch('/__atm/whoami')).status).toBe(404);
  });

  it('reads a tasks route’s flags as true only for "true" or "1", any case, and its single filters empty as none', () => {
    const filters = (query: string) => callFor(`/api/v1/projects/p1/tasks${query}`)!.args.filters;
    expect(filters('')).toEqual({
      sprint: [],
      backlog: false,
      release: undefined,
      component: undefined,
      status: [],
      type: undefined,
      priority: undefined,
      label: [],
      agent: undefined,
      ready: false,
      blocked: false,
      in_pr: false,
      with_user: false,
      archived: false,
    });
    for (const [value, set] of [
      ['true', true],
      ['TRUE', true],
      ['1', true],
      ['', false],
      ['yes', false],
      ['0', false],
      ['false', false],
    ] as const)
      for (const flag of ['backlog', 'ready', 'blocked', 'in_pr', 'with_user', 'archived'])
        expect((filters(`?${flag}=${value}`) as Record<string, unknown>)[flag], `${flag}=${value}`).toBe(set);
    expect(
      filters('?release=0.41.0&component=api&type=bug&priority=high&agent=claude%3Aa&label=x&label=y&release=0.42.0'),
    ).toMatchObject({
      release: '0.41.0',
      component: 'api',
      type: 'bug',
      priority: 'high',
      agent: 'claude:a',
      label: ['x', 'y'],
    });
    expect(filters('?release=&type=')).toMatchObject({ release: undefined, type: undefined });
  });

  it('a document or file route without a path asks for the empty one (the command refuses it)', () => {
    expect(callFor('/api/v1/projects/p1/documents')!.args).toEqual({
      workspace: 'default',
      project: 'p1',
      path: '',
      at: undefined,
    });
    expect(callFor('/api/v1/projects/p1/files')!.args.path).toBe('');
    expect(callFor('/api/search')!.args.q).toBe('');
    expect(callFor('/api/v2/workspaces/w/search')!.args.q).toBe('');
  });

  it.each([
    ['not_found', 404],
    ['invalid', 400],
    ['refused', 403],
    ['too_large', 413],
    ['conflict', 409],
    ['failed', 500],
    ['something new', 500],
  ])('a command’s %s error is HTTP %i with its message', async (kind, status) => {
    mockIPC(() => {
      throw { kind, message: `it was ${kind}` };
    });
    const resp = await ipcFetch('/api/projects');
    expect([resp.status, await resp.json()]).toEqual([status, { error: `it was ${kind}` }]);
  });

  it('an error that isn’t the app’s is a 500 with its text', async () => {
    mockIPC(() => {
      throw 'the app went away';
    });
    const resp = await ipcFetch('/api/projects');
    expect([resp.status, await resp.json()]).toEqual([500, { error: 'the app went away' }]);
  });

  it('an image command answering nothing is a 404; a JSON one answering nothing is null', async () => {
    mockIPC(() => null);
    const logo = await ipcFetch('/api/v1/projects/p1/logo');
    expect([logo.status, await logo.json()]).toEqual([404, { error: 'no logo' }]);
    const claims = await ipcFetch('/api/v1/projects/p1/claims');
    expect([claims.status, claims.headers.get('Content-Type'), await claims.json()]).toEqual([
      200,
      'application/json',
      null,
    ]);
  });

  it('takes its URL as a string, a URL or a Request', async () => {
    const asked: string[] = [];
    mockIPC((command) => {
      asked.push(command);
      return [];
    });
    await ipcFetch('/api/projects');
    await ipcFetch(new URL('http://localhost/api/search?q=x'));
    await ipcFetch(new Request('http://localhost/api/v2/workspaces'));
    expect(asked).toEqual(['list_projects', 'search', 'list_workspaces']);
  });
});
