// @vitest-environment node
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { BACKLOG } from '../../src/lib/filters';
import { buildBoard, buildLanes, readiness } from '../../src/lib/kanban';
import { sprintModel } from '../../src/lib/sprints';
import type { Task, TasksFile } from '../../src/lib/types';
import { resolveVocabulary } from '../../src/lib/vocab';
import sprintsData from '../fixtures/sprints.json';
import standard from '../fixtures/standard.json';

const sprintsFixture = sprintsData as TasksFile;

const t = (id: string, status: string, extra: Partial<Task> = {}): Task => ({
  id,
  name: id,
  phase: 0,
  type: 'feature',
  description: '',
  status,
  dependencies: [],
  ...extra,
});
const ids = (tasks: { task: Task }[]) => tasks.map((c) => c.task.id);
const all = { showSkipped: false, statusFilter: new Set<string>() };

describe('readiness', () => {
  const map = new Map([t('a', 'done'), t('b', 'todo')].map((x) => [x.id, x]));

  it('is ready when every dependency is done', () => {
    expect(readiness(t('x', 'todo', { dependencies: ['a'] }), map)).toEqual({ ready: true });
    expect(readiness(t('x', 'todo'), map)).toEqual({ ready: true });
  });

  it('lists unfinished and missing dependencies', () => {
    expect(readiness(t('x', 'todo', { dependencies: ['a', 'b', 'gone'] }), map)).toEqual({
      ready: false,
      waitingOn: ['b', 'gone'],
    });
  });

  it('takes atm’s word when the API gives it (T-366): an archived dependency it can’t see is done', () => {
    const archivedDep = { kind: 'todo', ready: true, waiting_for: [], priority: 'P2', archived: false };
    expect(readiness(t('x', 'todo', { dependencies: ['gone'], derived: archivedDep }), map)).toEqual({ ready: true });
    const waiting = { ...archivedDep, ready: false, waiting_for: ['b'] };
    expect(readiness(t('x', 'todo', { dependencies: ['a', 'b'], derived: waiting }), map)).toEqual({
      ready: false,
      waitingOn: ['b'],
    });
  });

  it('judges itself only a task without atm’s values (a linked worktree’s copy, or empty ones)', () => {
    const empty = {} as Task['derived'];
    expect(readiness(t('x', 'todo', { dependencies: ['b'], derived: empty }), map)).toEqual({
      ready: false,
      waitingOn: ['b'],
    });
  });
});

describe('Next up: atm says which task it is (T-321)', () => {
  const tasks = [t('T-1', 'todo'), t('T-2', 'todo'), t('T-3', 'in_review', { review_state: 'changes_requested' })];
  const marked = (nextUp: string | null | undefined) =>
    buildBoard(tasks, tasks, { ...all, nextUp })
      .flatMap((c) => c.cards)
      .filter((c) => c.nextUp)
      .map((c) => c.task.id);

  it("marks the task the project summary's next names, whichever column it is in", () => {
    expect(marked('T-2')).toEqual(['T-2']);
    // A task back from review, which atm next hands out first
    expect(marked('T-3')).toEqual(['T-3']);
  });

  it('marks nothing without one (nothing ready, or a summary without next)', () => {
    expect(marked(null)).toEqual([]);
    expect(marked(undefined)).toEqual([]);
  });

  it('is the rule /atm:loop-tasks uses: the skill takes its tasks from `atm next`', () => {
    // The rule lives in atm-cli only (claims.py loop_order): atm next hands out the first task, atm serve's summary
    // lists them as next, and the board marks that one. If this fails, the skill stopped using atm next.
    const skill = readFileSync(new URL('../../../plugin/skills/loop-tasks/SKILL.md', import.meta.url), 'utf8');
    expect(skill).toContain('`atm next --claim --json [selection] [--type T]`');
    expect(skill).toContain('to do, all dependencies done, not claimed');
    expect(skill).toContain('then the most urgent priority, then lowest task ID');
  });
});

describe('buildBoard', () => {
  it("has the vocabulary's columns by default (Skipped hidden), with counts", () => {
    const board = buildBoard(standard.tasks as Task[], standard.tasks as Task[], all);
    expect(board.map((c) => [c.label, c.cards.length])).toEqual([
      ['To Do', 4],
      ['In Progress', 1],
      ['In Review', 0],
      ['Blocked', 1],
      ['Done', 5],
    ]);
  });

  it('shows Skipped on request and Other only for unknown statuses', () => {
    const tasks = [t('a', 'skipped'), t('b', 'review'), t('c', 'todo')];
    expect(buildBoard(tasks, tasks, all).map((c) => c.id)).toEqual([
      'todo',
      'in_progress',
      'in_review',
      'blocked',
      'done',
      'other',
    ]);
    expect(buildBoard(tasks, tasks, { ...all, showSkipped: true }).map((c) => c.id)).toContain('skipped');
    expect(buildBoard([tasks[2]], tasks, all).map((c) => c.id)).not.toContain('other');
  });

  it('keeps every column under a status filter, adding Skipped when the filter names it', () => {
    const tasks = standard.tasks as Task[];
    const done = tasks.filter((x) => x.status === 'done');
    const board = buildBoard(done, tasks, { ...all, statusFilter: new Set(['done', 'skipped']) });
    expect(board.map((c) => c.id)).toEqual(['todo', 'in_progress', 'in_review', 'blocked', 'done', 'skipped']);
    expect(board.find((c) => c.id === 'in_review')!.cards).toEqual([]);
    expect(buildBoard(done, tasks, { ...all, statusFilter: new Set(['done']) }).map((c) => c.id)).not.toContain(
      'skipped',
    );
  });

  it('orders Todo ready-first, then by priority and ID', () => {
    const tasks = [
      t('T-1', 'todo', { dependencies: ['T-9'] }),
      t('T-3', 'todo'),
      t('T-2', 'todo'),
      t('T-4', 'todo', { priority: 'P0' }),
      t('T-9', 'in_progress'),
    ];
    const todo = buildBoard(tasks, tasks, all)[0];
    expect(ids(todo.cards)).toEqual(['T-4', 'T-2', 'T-3', 'T-1']);
    expect(todo.cards.map((c) => c.readiness?.ready)).toEqual([true, true, true, false]);
  });

  it('orders In Progress by start and Done by completion, newest first, with fallbacks', () => {
    const tasks = [
      t('a', 'in_progress', { started_at: '2026-04-01T00:00:00Z' }),
      t('b', 'in_progress', { updated_at: '2026-04-03T00:00:00Z' }),
      t('c', 'done', { completed_at: '2026-04-02T00:00:00Z', updated_at: '2026-04-09T00:00:00Z' }),
      t('d', 'done', { updated_at: '2026-04-05T00:00:00Z' }),
      t('e', 'done'),
    ];
    const board = buildBoard(tasks, tasks, all);
    expect(ids(board.find((c) => c.id === 'in_progress')!.cards)).toEqual(['b', 'a']);
    expect(ids(board.find((c) => c.id === 'done')!.cards)).toEqual(['d', 'c', 'e']);
  });

  it('judges readiness against all tasks, not just the visible ones', () => {
    const allTasks = [t('T-1', 'todo'), t('T-2', 'todo', { dependencies: ['T-3'] }), t('T-3', 'done')];
    const todo = buildBoard([allTasks[1]], allTasks, { ...all, nextUp: 'T-1' })[0];
    expect(todo.cards[0].readiness).toEqual({ ready: true });
    // T-1 (hidden by the filter) is still the loop's next pick
    expect(todo.cards[0].nextUp).toBe(false);
  });
});

describe('a custom vocabulary', () => {
  const vocab = resolveVocabulary({
    statuses: [
      { id: 'backlog', label: 'Backlog', kind: 'todo' },
      { id: 'doing', label: 'Doing', kind: 'active' },
      { id: 'awaiting_user', label: 'Awaiting you', kind: 'review' },
      { id: 'shipped', label: 'Shipped', kind: 'done' },
      { id: 'dropped', label: 'Dropped', kind: 'skipped' },
    ],
  });
  const tasks = [
    t('T-1', 'shipped', { phase: 0 }),
    t('T-2', 'backlog', { phase: 0, dependencies: ['T-1'] }),
    t('T-3', 'backlog', { phase: 0, dependencies: ['T-4'] }),
    t('T-4', 'awaiting_user', { phase: 0 }),
    t('T-5', 'dropped', { phase: 0 }),
    t('T-6', 'todo', { phase: 0 }),
  ];
  const options = { ...all, vocab };

  it('makes its columns, in its order and with its labels; skipped-kind columns behind the toggle', () => {
    const board = buildBoard(tasks, tasks, options);
    expect(board.map((c) => [c.id, c.label, c.kind])).toEqual([
      ['backlog', 'Backlog', 'todo'],
      ['doing', 'Doing', 'active'],
      ['awaiting_user', 'Awaiting you', 'review'],
      ['shipped', 'Shipped', 'done'],
      // `todo` isn't one of its statuses
      ['other', 'Other', undefined],
    ]);
    expect(buildBoard(tasks, tasks, { ...options, showSkipped: true }).map((c) => c.id)).toContain('dropped');
  });

  it('judges readiness by kind: done-kind meets a dependency, review-kind does not', () => {
    const backlog = buildBoard(tasks, tasks, options)[0];
    expect(backlog.cards.map((c) => [c.task.id, c.readiness])).toEqual([
      ['T-2', { ready: true }],
      ['T-3', { ready: false, waitingOn: ['T-4'] }],
    ]);
  });

  it('counts done-kind tasks as done in lanes', () => {
    expect(buildLanes(tasks, tasks, options)[0].done).toBe(1);
  });
});

describe('buildLanes', () => {
  it('makes one lane per sprint with progress, all with the same columns (phases read as sprints)', () => {
    const tasks = [
      t('a', 'done', { phase: 0 }),
      t('b', 'todo', { phase: 0 }),
      t('c', 'review', { phase: 1 }),
      t('d', 'in_progress', { phase: 1 }),
    ];
    const sprints = sprintModel({}, tasks, resolveVocabulary({}));
    const lanes = buildLanes(tasks, tasks, { ...all, sprints });
    expect(lanes.map((l) => [l.key, l.index, l.done, l.total])).toEqual([
      ['Phase 0', 0, 1, 2],
      ['Phase 1', 1, 0, 2],
    ]);
    // "Other" exists because of Phase 1; Phase 0 gets it too (empty) so lanes line up
    for (const lane of lanes)
      expect(lane.columns.map((c) => c.id)).toEqual(['todo', 'in_progress', 'in_review', 'blocked', 'done', 'other']);
    expect(lanes[0].columns.find((c) => c.id === 'other')!.cards).toEqual([]);
    expect(ids(lanes[1].columns.find((c) => c.id === 'other')!.cards)).toEqual(['c']);
  });

  it("follows the project's sprint order, leaves out sprints without tasks, and puts the backlog last (SR-7.3)", () => {
    const tasks = sprintsFixture.tasks as Task[];
    const vocab = resolveVocabulary(sprintsFixture.meta);
    const sprints = sprintModel(sprintsFixture.meta, tasks, vocab);
    const lanes = buildLanes(tasks, tasks, { ...all, vocab, sprints });
    expect(lanes.map((l) => [l.key, l.index, l.done, l.total])).toEqual([
      ['Foundations', 0, 1, 1],
      ['Ada Lovelace', 1, 0, 3],
      [BACKLOG, -1, 0, 1],
    ]);
  });

  it('without sprints or phases, every task is in the backlog lane', () => {
    const tasks = [{ id: 'a', name: 'a', type: 'feature', description: '', status: 'todo' } as Task];
    expect(buildLanes(tasks, tasks, all).map((l) => [l.key, l.total])).toEqual([[BACKLOG, 1]]);
  });
});
