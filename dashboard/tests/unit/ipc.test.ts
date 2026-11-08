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
    for (const url of ['/__atm/whoami', '/p/p1/tasks.json', '/api/v2/workspaces/w/projects/p1/tasks/x', '/api/v2'])
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
});
