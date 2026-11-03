// A project's git worktrees on one board (T-179). Each worktree has its branch's copy of tasks.json; atm serve's
// p/<id>/worktrees.json lists the worktrees and, for each linked one, the tasks whose copy there differs from the main
// worktree's, as views ({task, derived}) with atm's derived values worked out in that worktree (T-387): a copy is
// judged by atm, like every other task. A task is shown from the worktree where it got furthest, with that
// worktree's branch on its card.
import { browserFetch, fetchProjectFile, type Fetch, type Validators } from './api/http';
import type { Task } from './types';
import { kindOf, type StatusKind, type Vocabulary } from './vocab';

export interface Worktree {
  path: string;
  branch: string | null;
  main: boolean;
  /** Linked worktrees only: the tasks that differ from the main worktree's copy, or that only this one has, each with
   * atm's derived values from that worktree */
  tasks?: Task[];
}

/** A worktree's views as tasks, each with its derived values beside its fields (as the v1 tasks route's, load.ts) */
function viewedCopies(list: unknown[]): Task[] {
  return list
    .filter((v): v is { task: Task; derived?: Task['derived'] } => !!v && typeof v === 'object' && 'task' in v)
    .filter((v) => !!v.task && typeof v.task === 'object')
    .map((v) => (v.derived ? { ...v.task, derived: v.derived } : v.task));
}

/** How far a task got, by its status's kind: a linked worktree's copy wins only when it got further */
const PROGRESS: Record<StatusKind, number> = { todo: 0, blocked: 1, active: 2, review: 3, done: 4, skipped: 4 };

function progress(vocab: Vocabulary, task: Task): number {
  const kind = kindOf(vocab, task.status);
  return kind ? PROGRESS[kind] : -1;
}

export interface Merged {
  tasks: Task[];
  /** The branch each task is shown from, for tasks that come from a linked worktree */
  branches: ReadonlyMap<string, string>;
}

/**
 * The main worktree's tasks, each replaced by a linked worktree's copy that got further (the furthest one; the first
 * listed on a tie), plus the tasks only linked worktrees have, after the others in ID order.
 */
export function mergeWorktrees(tasks: Task[], worktrees: readonly Worktree[], vocab: Vocabulary): Merged {
  const shown = new Map(tasks.map((t) => [t.id, t]));
  const branches = new Map<string, string>();
  for (const tree of worktrees) {
    if (tree.main) continue;
    for (const task of tree.tasks ?? []) {
      if (!task || typeof task.id !== 'string') continue;
      const current = shown.get(task.id);
      if (current && progress(vocab, task) <= progress(vocab, current)) continue;
      shown.set(task.id, task);
      branches.set(task.id, tree.branch ?? tree.path);
    }
  }
  if (!branches.size) return { tasks, branches };
  const known = new Set(tasks.map((t) => t.id));
  const added = [...shown.values()]
    .filter((t) => !known.has(t.id))
    .sort((a, b) => a.id.localeCompare(b.id, undefined, { numeric: true }));
  return { tasks: [...tasks.map((t) => shown.get(t.id)!), ...added], branches };
}

export type WorktreesResult = { unchanged: true } | { unchanged: false; worktrees: Worktree[]; validators: Validators };

/**
 * Fetches a project's worktrees.json, conditionally when `since` is given. A server without it (404) means no
 * linked worktrees; malformed entries are left out.
 */
export async function loadWorktrees(
  project: string,
  fetchFn: Fetch = browserFetch,
  since?: Validators,
): Promise<WorktreesResult> {
  const file = await fetchProjectFile(project, 'worktrees.json', fetchFn, since, '{"worktrees": []}');
  if (file.unchanged) return file;
  const list = (JSON.parse(file.text) as { worktrees?: unknown }).worktrees;
  const worktrees = (Array.isArray(list) ? list : [])
    .filter((w): w is Worktree => !!w && typeof w === 'object' && typeof (w as Worktree).path === 'string')
    .map((w) => ({
      path: w.path,
      branch: typeof w.branch === 'string' && w.branch ? w.branch : null,
      main: w.main === true,
      ...(Array.isArray(w.tasks) ? { tasks: viewedCopies(w.tasks) } : {}),
    }));
  return { unchanged: false, worktrees, validators: file.validators };
}
