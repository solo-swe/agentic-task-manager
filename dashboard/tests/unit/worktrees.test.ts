import { describe, expect, it, vi } from 'vitest';
import type { Task } from '../../src/lib/types';
import { DEFAULT_VOCABULARY, type Vocabulary } from '../../src/lib/vocab';
import { loadWorktrees, mergeWorktrees, type Worktree } from '../../src/lib/worktrees';
import { withKinds } from './views';

/** A task as atm serves it: with its kind in `vocab` */
const task = (id: string, status: string, extra: Partial<Task> = {}, vocab: Vocabulary = DEFAULT_VOCABULARY): Task =>
  withKinds([{ id, name: id, phase: 1, type: 'feature', description: '', status, ...extra } as Task], vocab)[0];
const main: Worktree = { path: '/p', branch: 'main', main: true };
/** A linked worktree with its differing copies, and the IDs atm shows from it (ATM-724: atm ranks the copies) */
const linked = (branch: string, tasks: Task[], shown?: string[]): Worktree => ({
  path: `/p-${branch}`,
  branch,
  main: false,
  tasks,
  ...(shown ? { shown } : {}),
});

describe('mergeWorktrees', () => {
  it('without linked worktrees: the main tasks as they are, and no branches', () => {
    const tasks = [task('T-1', 'todo')];
    const merged = mergeWorktrees(tasks, [main]);
    expect(merged.tasks).toBe(tasks);
    expect(merged.branches.size).toBe(0);
    expect(mergeWorktrees(tasks, [main, linked('a', [])]).tasks).toBe(tasks);
  });

  it("shows a task from the worktree atm names, with that worktree's branch", () => {
    const tasks = [task('T-1', 'todo'), task('T-2', 'todo'), task('T-3', 'done')];
    const merged = mergeWorktrees(tasks, [
      main,
      linked('agent-1', [task('T-1', 'in_progress'), task('T-3', 'todo')], []),
      linked('agent-2', [task('T-1', 'done'), task('T-2', 'in_review')], ['T-1', 'T-2']),
    ]);
    expect(merged.tasks.map((t) => [t.id, t.status])).toEqual([
      ['T-1', 'done'],
      ['T-2', 'in_review'],
      ['T-3', 'done'], // atm shows the main worktree's copy: it stays, without a branch
    ]);
    expect([...merged.branches]).toEqual([
      ['T-1', 'agent-2'],
      ['T-2', 'agent-2'],
    ]);
  });

  it('a copy atm doesn’t name is never shown, whatever its status: the ranking is atm’s (ATM-724)', () => {
    const tasks = [task('T-1', 'todo')];
    // Done in the linked worktree, but atm shows main's: the dashboard never ranks the copies itself
    expect(mergeWorktrees(tasks, [main, linked('a', [task('T-1', 'done')], [])]).tasks).toBe(tasks);
    // A server before ATM-724 (no `shown`): main's copies, as nothing says otherwise
    expect(mergeWorktrees(tasks, [main, linked('a', [task('T-1', 'done')])]).tasks).toBe(tasks);
    // An ID atm names that the worktree's copies don't have is ignored
    expect(mergeWorktrees(tasks, [main, linked('a', [], ['T-1'])]).tasks).toBe(tasks);
  });

  it('adds the tasks only a linked worktree has, in ID order, after the others', () => {
    const merged = mergeWorktrees(
      [task('T-1', 'todo')],
      [main, linked('a', [task('T-10', 'todo'), task('T-9', 'in_progress')], ['T-10', 'T-9'])],
    );
    expect(merged.tasks.map((t) => t.id)).toEqual(['T-1', 'T-9', 'T-10']);
    expect(merged.branches.get('T-10')).toBe('a');
  });

  it('a detached worktree is named by its folder', () => {
    const detached: Worktree = {
      path: '/p-tmp',
      branch: null,
      main: false,
      tasks: [task('T-1', 'done')],
      shown: ['T-1'],
    };
    const merged = mergeWorktrees([task('T-1', 'in_progress')], [main, detached]);
    expect(merged.tasks[0].status).toBe('done');
    expect(merged.branches.get('T-1')).toBe('/p-tmp');
  });
});

describe('loadWorktrees', () => {
  it("fetches the project's worktrees.json, conditionally once it has validators", async () => {
    // A linked worktree's copies come as views, with atm's derived values from that worktree (T-387)
    const derived = { kind: 'done', ready: false, waiting_for: [], priority: 'P2', archived: false, stage: 'done' };
    const views = [{ task: task('T-1', 'done'), derived }, { derived }, null];
    const body = {
      worktrees: [main, { ...linked('a', []), tasks: views, shown: ['T-1', 7], extra: 1 }, { branch: 'x' }],
    };
    const fetchFn = vi.fn().mockResolvedValue(new Response(JSON.stringify(body), { headers: { ETag: '"w1"' } }));
    const result = await loadWorktrees('abcd1234', fetchFn);
    expect(fetchFn).toHaveBeenCalledWith('/p/abcd1234/worktrees.json', { cache: 'no-store', headers: {} });
    expect(result).toEqual({
      unchanged: false,
      // atm's `shown` kept (its IDs only)
      worktrees: [main, linked('a', [{ ...task('T-1', 'done'), derived }], ['T-1'])],
      validators: { etag: '"w1"' },
    });
    const again = vi.fn().mockResolvedValue(new Response(null, { status: 304 }));
    expect(await loadWorktrees('abcd1234', again, { etag: '"w1"' })).toEqual({ unchanged: true });
  });

  it('a server without worktrees.json (404) means no linked worktrees', async () => {
    const result = await loadWorktrees('x', vi.fn().mockResolvedValue(new Response('nope', { status: 404 })));
    expect(result).toEqual({ unchanged: false, worktrees: [], validators: {} });
  });
});
