import { describe, expect, it, vi } from 'vitest';
import type { Task } from '../../src/lib/types';
import { DEFAULT_VOCABULARY, resolveVocabulary } from '../../src/lib/vocab';
import { loadWorktrees, mergeWorktrees, type Worktree } from '../../src/lib/worktrees';

const task = (id: string, status: string, extra: Partial<Task> = {}): Task => ({
  id,
  name: id,
  phase: 1,
  type: 'feature',
  description: '',
  status,
  ...extra,
});
const main: Worktree = { path: '/p', branch: 'main', main: true };
const linked = (branch: string, tasks: Task[]): Worktree => ({ path: `/p-${branch}`, branch, main: false, tasks });

describe('mergeWorktrees', () => {
  it('without linked worktrees: the main tasks as they are, and no branches', () => {
    const tasks = [task('T-1', 'todo')];
    const merged = mergeWorktrees(tasks, [main], DEFAULT_VOCABULARY);
    expect(merged.tasks).toBe(tasks);
    expect(merged.branches.size).toBe(0);
    expect(mergeWorktrees(tasks, [main, linked('a', [])], DEFAULT_VOCABULARY).tasks).toBe(tasks);
  });

  it("shows a task from the worktree where it got furthest, with that worktree's branch", () => {
    const tasks = [task('T-1', 'todo'), task('T-2', 'todo'), task('T-3', 'done')];
    const merged = mergeWorktrees(
      tasks,
      [
        main,
        linked('agent-1', [task('T-1', 'in_progress'), task('T-3', 'todo')]),
        linked('agent-2', [task('T-1', 'done'), task('T-2', 'in_review')]),
      ],
      DEFAULT_VOCABULARY,
    );
    expect(merged.tasks.map((t) => [t.id, t.status])).toEqual([
      ['T-1', 'done'],
      ['T-2', 'in_review'],
      ['T-3', 'done'], // the main worktree's copy got further: it stays, without a branch
    ]);
    expect([...merged.branches]).toEqual([
      ['T-1', 'agent-2'],
      ['T-2', 'agent-2'],
    ]);
  });

  it("a tie keeps the main worktree's copy, or the first linked one's", () => {
    const merged = mergeWorktrees(
      [task('T-1', 'in_progress'), task('T-2', 'todo')],
      [
        main,
        linked('a', [task('T-1', 'in_progress', { name: 'renamed' }), task('T-2', 'in_progress')]),
        linked('b', [task('T-2', 'in_progress', { name: 'other' })]),
      ],
      DEFAULT_VOCABULARY,
    );
    expect(merged.tasks.map((t) => t.name)).toEqual(['T-1', 'T-2']);
    expect([...merged.branches]).toEqual([['T-2', 'a']]);
  });

  it('adds the tasks only a linked worktree has, in ID order, after the others', () => {
    const merged = mergeWorktrees(
      [task('T-1', 'todo')],
      [main, linked('a', [task('T-10', 'todo'), task('T-9', 'in_progress')])],
      DEFAULT_VOCABULARY,
    );
    expect(merged.tasks.map((t) => t.id)).toEqual(['T-1', 'T-9', 'T-10']);
    expect(merged.branches.get('T-10')).toBe('a');
  });

  it("goes by the project's status kinds; a detached worktree is named by its folder", () => {
    const vocab = resolveVocabulary({
      statuses: [
        { id: 'open', label: 'Open', kind: 'todo' },
        { id: 'doing', label: 'Doing', kind: 'active' },
        { id: 'shipped', label: 'Shipped', kind: 'done' },
      ],
    });
    const detached: Worktree = { path: '/p-tmp', branch: null, main: false, tasks: [task('T-1', 'shipped')] };
    const merged = mergeWorktrees([task('T-1', 'doing')], [main, detached], vocab);
    expect(merged.tasks[0].status).toBe('shipped');
    expect(merged.branches.get('T-1')).toBe('/p-tmp');
  });
});

describe('loadWorktrees', () => {
  it("fetches the project's worktrees.json, conditionally once it has validators", async () => {
    // A linked worktree's copies come as views, with atm's derived values from that worktree (T-387)
    const derived = { kind: 'done', ready: false, waiting_for: [], priority: 'P2', archived: false, stage: 'done' };
    const views = [{ task: task('T-1', 'done'), derived }, { derived }, null];
    const body = { worktrees: [main, { ...linked('a', []), tasks: views, extra: 1 }, { branch: 'x' }] };
    const fetchFn = vi.fn().mockResolvedValue(new Response(JSON.stringify(body), { headers: { ETag: '"w1"' } }));
    const result = await loadWorktrees('abcd1234', fetchFn);
    expect(fetchFn).toHaveBeenCalledWith('/p/abcd1234/worktrees.json', { cache: 'no-store', headers: {} });
    expect(result).toEqual({
      unchanged: false,
      worktrees: [main, linked('a', [{ ...task('T-1', 'done'), derived }])],
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
