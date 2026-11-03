import { describe, expect, it } from 'vitest';
import { allBoard, allBoardLanes, BOARD_COLUMNS, boardOf, withoutActiveSprint } from '../../src/lib/allboard';
import type { BoardCard, BoardColumnId, ProjectEntry } from '../../src/lib/load';
import { emptyBoard, olderServer, summary } from './summary.helpers';

const card = (id: string, fields: Partial<BoardCard> = {}): BoardCard => ({
  id,
  name: id,
  type: 'feature',
  sprint: null,
  priority: 'P2',
  labels: [],
  ...fields,
});

function project(
  id: string,
  lastActivity: string | null,
  columns: Partial<Record<BoardColumnId, BoardCard[]>>,
  counts: Partial<Record<BoardColumnId, number>> = {},
  extra: Partial<ProjectEntry> = {},
): ProjectEntry {
  const board = emptyBoard();
  for (const { id: c } of BOARD_COLUMNS) {
    board[c] = { count: counts[c] ?? (columns[c] ?? []).length, cards: columns[c] ?? [] };
  }
  return {
    id,
    name: id,
    path: `/${id}`,
    missing: false,
    summary: summary({ last_activity: lastActivity, board }),
    ...extra,
  };
}

const ids = (columns: ReturnType<typeof allBoard>, id: BoardColumnId) =>
  columns.find((c) => c.id === id)!.cards.map((c) => `${c.project.id}/${c.card.id}`);

describe('allBoard', () => {
  it('has the five columns, empty ones saying why', () => {
    const columns = allBoard([]);
    expect(columns.map((c) => c.id)).toEqual(['todo', 'active', 'review', 'blocked', 'done']);
    expect(columns[0]).toMatchObject({ cards: [], more: [], empty: 'Nothing to do' });
    expect(columns[4].empty).toBe('Nothing finished in the last 24 hours');
  });

  it('To Do: the most recently active project first, each in its own order', () => {
    const a = project('a', '2026-03-01T10:00:00Z', { todo: [card('A-2'), card('A-1')] });
    const b = project('b', '2026-03-01T11:00:00Z', { todo: [card('B-1')] });
    const never = project('n', null, { todo: [card('N-1')] });
    expect(ids(allBoard([never, a, b]), 'todo')).toEqual(['b/B-1', 'a/A-2', 'a/A-1', 'n/N-1']);
  });

  it('in progress and review: the longest first; unknown times last', () => {
    const a = project('a', null, {
      active: [card('A-1', { since: '2026-03-01T10:00:00Z' }), card('A-2', { since: null })],
      review: [card('A-3', { since: '2026-03-01T09:00:00Z' })],
    });
    const b = project('b', null, {
      active: [card('B-1', { since: '2026-03-01T08:00:00Z' })],
      review: [card('B-2', { since: '2026-03-01T10:00:00Z' })],
    });
    const columns = allBoard([a, b]);
    expect(ids(columns, 'active')).toEqual(['b/B-1', 'a/A-1', 'a/A-2']);
    expect(ids(columns, 'review')).toEqual(['a/A-3', 'b/B-2']);
  });

  it('blocked: on you first, then the longest waiting', () => {
    const a = project('a', null, {
      blocked: [
        card('A-1', { since: '2026-03-01T06:00:00Z', waiting_on: 'agent' }),
        card('A-2', { since: '2026-03-01T11:00:00Z', waiting_on: 'user' }),
      ],
    });
    const b = project('b', null, { blocked: [card('B-1', { since: '2026-03-01T09:00:00Z', waiting_on: 'user' })] });
    expect(ids(allBoard([a, b]), 'blocked')).toEqual(['b/B-1', 'a/A-2', 'a/A-1']);
  });

  it('done: the most recent first; unknown times last', () => {
    const a = project('a', null, {
      done: [card('A-1', { completed_at: '2026-03-01T10:00:00Z' }), card('A-2', { completed_at: null })],
    });
    const b = project('b', null, { done: [card('B-1', { completed_at: '2026-03-01T11:00:00Z' })] });
    expect(ids(allBoard([a, b]), 'done')).toEqual(['b/B-1', 'a/A-1', 'a/A-2']);
  });

  it('one cap per column; "+ N more" per project counts what the server and the cap left out', () => {
    const a = project('a', '2026-03-01T11:00:00Z', { todo: [card('A-1'), card('A-2')] }, { todo: 12 });
    const b = project('b', '2026-03-01T10:00:00Z', { todo: [card('B-1'), card('B-2')] });
    const todo = allBoard([a, b], 3)[0];
    expect(todo.cards.map((c) => c.card.id)).toEqual(['A-1', 'A-2', 'B-1']);
    expect(todo.more.map((m) => [m.project.id, m.count])).toEqual([
      ['a', 10],
      ['b', 1],
    ]);
  });

  it('grouped by project: a lane each, the most recently active first; no lane without cards (T-260)', () => {
    const a = project('a', '2026-03-01T10:00:00Z', { todo: [card('A-1')] });
    const b = project('b', '2026-03-01T11:00:00Z', { done: [card('B-1')] }, { done: 3 });
    const empty = project('e', '2026-03-01T12:00:00Z', {});
    const gone = project('g', null, { todo: [card('G-1')] }, {}, { missing: true });
    const lanes = allBoardLanes([a, b, empty, gone]);
    expect(lanes.map((l) => l.project.id)).toEqual(['b', 'a']);
    const done = lanes[0].columns.find((c) => c.id === 'done')!;
    expect([done.cards.length, done.more.map((m) => m.count)]).toEqual([1, [2]]);
  });

  it('a search keeps the matching cards, counts the whole column, and drops "+ N more" (T-261)', () => {
    const a = project('a', '2026-03-01T11:00:00Z', { todo: [card('A-1'), card('A-2')] }, { todo: 12 });
    const b = project('b', '2026-03-01T10:00:00Z', { todo: [card('B-1')], done: [card('B-2')] });
    const match = new Map([
      ['a', new Set(['A-2', 'A-9'])],
      ['b', new Set(['B-2'])],
    ]);
    const [todo, , , , done] = allBoard([a, b], undefined, match);
    expect(todo.cards.map((c) => c.card.id)).toEqual(['A-2']);
    expect([todo.total, todo.more, todo.empty]).toEqual([13, [], 'No match']);
    expect(done.cards.map((c) => c.card.id)).toEqual(['B-2']);
    // Grouped: a project without a matching card gets no lane
    expect(allBoardLanes([a, b], undefined, new Map([['b', new Set(['B-2'])]])).map((l) => l.project.id)).toEqual([
      'b',
    ]);
  });

  it('projects without a board (gone, or not summarized) add nothing', () => {
    const gone = project('g', null, { todo: [card('G-1')] }, {}, { missing: true });
    const unsummarized = { ...project('o', null, {}), summary: null };
    expect(allBoard([gone, unsummarized]).every((c) => c.cards.length === 0 && c.more.length === 0)).toBe(true);
  });
});

describe('the active sprints or all tasks (T-340)', () => {
  /** A project whose whole board has `all` and whose active sprint's board has `active` (null: no active sprint) */
  function scoped(id: string, all: Partial<Record<BoardColumnId, BoardCard[]>>, active: typeof all | null) {
    const whole = project(id, '2026-05-01T10:00:00Z', all);
    const sprint = active && project(id, null, active).summary!.board!;
    return { ...whole, summary: { ...whole.summary!, board_active_sprint: sprint } } as ProjectEntry;
  }
  const a = scoped('a', { todo: [card('A-1'), card('A-2')], blocked: [card('A-3')] }, { todo: [card('A-1')] });
  const b = scoped('b', { todo: [card('B-1')] }, null);

  it('the active scope shows each project’s active sprint board, with its own counts; all, the whole board', () => {
    const active = allBoard([a, b], undefined, null, 'active');
    expect(ids(active, 'todo')).toEqual(['a/A-1']);
    expect(active.find((c) => c.id === 'todo')!.total).toBe(1);
    expect(ids(active, 'blocked')).toEqual([]);
    const all = allBoard([a, b], undefined, null, 'all');
    expect(ids(all, 'todo')).toEqual(['a/A-1', 'a/A-2', 'b/B-1']);
    expect(ids(all, 'blocked')).toEqual(['a/A-3']);
  });

  it('grouped by project: no lane for a project without an active sprint', () => {
    expect(allBoardLanes([a, b], undefined, null, 'active').map((l) => l.project.id)).toEqual(['a']);
    expect(allBoardLanes([a, b], undefined, null, 'all').map((l) => l.project.id)).toEqual(['a', 'b']);
  });

  it('the active scope’s Done is every done task of the sprints, not the last 24 hours’ (T-349)', () => {
    const done = (scope: 'active' | 'all') => allBoard([a, b], 30, null, scope).find((c) => c.id === 'done')!;
    expect([done('active').label, done('active').empty]).toEqual(['Done', 'Nothing done in it yet']);
    expect(done('all').label).toBe('Done · last 24 h');
    expect(allBoard([a], 30, null, 'active')[0].label).toBe('To Do');
  });

  it('names the projects without an active sprint, only in the active scope', () => {
    expect(withoutActiveSprint([a, b], 'active').map((p) => p.id)).toEqual(['b']);
    expect(withoutActiveSprint([a, b], 'all')).toEqual([]);
  });

  it('an older server without the active sprint’s board shows the whole board, and is not listed as without one', () => {
    const full = project('o', null, { todo: [card('O-1')] });
    const older = { ...full, summary: olderServer(full.summary!, 'board_active_sprint') };
    expect(boardOf(older, 'active')).toBe(older.summary!.board);
    expect(withoutActiveSprint([older], 'active')).toEqual([]);
    expect(boardOf({ ...older, missing: true }, 'active')).toBeNull();
  });
});
