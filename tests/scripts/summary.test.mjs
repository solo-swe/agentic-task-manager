// `atm summary`: the project's summary as atm serve lists it (docs/multi-project-api.md 3.2). Its `next` is what the
// dashboard marks Next up (T-321), so it must follow `atm next`'s own rule: what scenarios can't say, as the summary
// holds times and counts that vary.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { makeProject, removeDir, runAtm } from './helpers.mjs';

const FILE = '.agentic-task-manager/tasks.json';

function withProject(data, check) {
  const root = makeProject({ files: { [FILE]: data } });
  try {
    check(root);
  } finally {
    removeDir(root);
  }
}

function summary(root) {
  const r = runAtm(['project', 'summary'], root);
  assert.equal(r.code, 0, r.stderr);
  return JSON.parse(r.stdout);
}

function atmNext(root) {
  const r = runAtm(['loop', 'next', '--json'], root);
  assert.ok(r.code === 0 || r.code === 3, r.stderr);
  return JSON.parse(r.stdout).task?.id ?? null;
}

const task = (id, status, extra = {}) => ({ id, name: id, type: 'feature', status, ...extra });

test("next: back from review first, work together with the user skipped, then priority and ID; atm next's first", () => {
  const tasks = [
    task('T-1', 'todo', { priority: 'P0', with: 'user' }),
    task('T-2', 'todo', { priority: 'P3' }),
    task('T-3', 'todo', { priority: 'P0' }),
    task('T-4', 'in_review', { review_state: 'changes_requested' }),
    task('T-5', 'in_review', { review_state: 'waiting' }),
  ];
  withProject({ tasks }, (root) => {
    const s = summary(root);
    assert.deepEqual(
      s.next.map((t) => t.id),
      ['T-4', 'T-3'],
    );
    assert.equal(atmNext(root), s.next[0].id);
    // The board's To Do column holds only to-do tasks, not the one back from review; the loop's first, then the one
    // done with the user (T-349)
    assert.deepEqual(
      s.board.todo.cards.map((c) => c.id),
      ['T-3', 'T-2', 'T-1'],
    );
    assert.equal(s.board.todo.cards[2].with_user, true);
  });
});

test('next: the active sprint only, even before a more urgent backlog task; none without an active sprint', () => {
  const tasks = [
    task('T-1', 'todo', { priority: 'P0' }),
    task('T-2', 'todo', { sprint: 'Later' }),
    task('T-3', 'todo', { sprint: 'Now', priority: 'P3' }),
    task('T-4', 'todo', { sprint: 'Now' }),
  ];
  const sprints = [
    { id: 'Now', state: 'active' },
    { id: 'Later', state: 'planned' },
  ];
  withProject({ meta: { sprints }, tasks }, (root) => {
    const s = summary(root);
    assert.deepEqual(
      s.next.map((t) => t.id),
      ['T-4', 'T-3'],
    );
    assert.equal(atmNext(root), 'T-4');
  });
  withProject({ meta: { sprints: [{ id: 'Now' }, { id: 'Later' }] }, tasks }, (root) => {
    assert.deepEqual(summary(root).next, []);
    assert.equal(atmNext(root), null);
  });
});

test('the board’s To Do: every to-do task, the loop’s first, then with you, then waiting (T-349)', () => {
  const tasks = [
    task('T-1', 'todo', { sprint: 'Now', dependencies: ['T-9'] }),
    task('T-2', 'todo', { sprint: 'Now', with: 'user' }),
    task('T-3', 'todo', { sprint: 'Now', priority: 'P3' }),
    task('T-4', 'todo', { sprint: 'Later' }),
    task('T-5', 'todo'),
    task('T-9', 'in_progress', { sprint: 'Now' }),
    // Done long ago, in the sprint and outside it
    task('D-1', 'done', { sprint: 'Now', completed_at: '2020-01-01T00:00:00Z' }),
    task('D-2', 'done', { completed_at: '2020-01-01T00:00:00Z' }),
  ];
  const sprints = [
    { id: 'Now', state: 'active' },
    { id: 'Later', state: 'planned' },
  ];
  withProject({ meta: { sprints }, tasks }, (root) => {
    const s = summary(root);
    const ids = (column) => column.cards.map((c) => c.id);
    // The active sprint's board: every one of its tasks, no more, no less, done ones however long ago
    assert.deepEqual(ids(s.board_active_sprint.done), ['D-1']);
    // Every task: done in the last 24 hours only, as before
    assert.deepEqual(ids(s.board.done), []);
    // The active sprint's: the loop's T-3, then T-2 with you, then T-1 waiting on T-9
    assert.deepEqual(ids(s.board_active_sprint.todo), ['T-3', 'T-2', 'T-1']);
    assert.deepEqual(s.board_active_sprint.todo.cards[2].waiting_for, ['T-9']);
    assert.equal(s.board_active_sprint.todo.cards[0].waiting_for, undefined);
    // Every task: the other sprints' and the backlog's too, ready ones before the waiting one
    assert.deepEqual(ids(s.board.todo), ['T-3', 'T-4', 'T-5', 'T-2', 'T-1']);
    assert.equal(s.board.todo.count, 5);
  });
});

test('needs you: with-you tasks only in the active sprint and ready; reviews and blocks anywhere (T-347)', () => {
  const tasks = [
    task('W-1', 'todo', { with: 'user', sprint: 'Now' }),
    task('W-2', 'todo', { with: 'user', sprint: 'Later' }),
    task('W-3', 'todo', { with: 'user' }),
    task('W-4', 'todo', { with: 'user', sprint: 'Now', dependencies: ['D-1'] }),
    task('W-5', 'todo', { with: 'user', sprint: 'Now', dependencies: ['D-2'] }),
    task('D-1', 'in_progress', { sprint: 'Now' }),
    task('D-2', 'done', { sprint: 'Now' }),
    task('R-1', 'in_review', { sprint: 'Later' }),
    task('B-1', 'blocked', { waiting_on: 'user' }),
  ];
  const ids = (s) => s.needs_you_tasks.map((t) => `${t.id}:${t.reason}`);
  withProject(
    {
      meta: {
        sprints: [
          { id: 'Now', state: 'active' },
          { id: 'Later', state: 'planned' },
        ],
      },
      tasks,
    },
    (root) => {
      const s = summary(root);
      assert.deepEqual(ids(s), ['W-1:together', 'W-5:together', 'R-1:review', 'B-1:blocked']);
      assert.deepEqual(s.needs_you, { review: 1, blocked_on_user: 1, together: 2 });
    },
  );
  // No active sprint: any sprint's, still only when ready
  withProject({ meta: { sprints: [{ id: 'Now' }, { id: 'Later' }] }, tasks }, (root) => {
    assert.deepEqual(ids(summary(root)), [
      'W-1:together',
      'W-2:together',
      'W-3:together',
      'W-5:together',
      'R-1:review',
      'B-1:blocked',
    ]);
  });
});

test('board_active_sprint: the board of the active sprint’s tasks only, with its own counts; null without one (T-340)', () => {
  const done = { completed_at: new Date().toISOString().replace(/\.\d+Z$/, 'Z') };
  const tasks = [
    task('T-1', 'todo', { sprint: 'Now' }),
    task('T-2', 'in_progress', { sprint: 'Now' }),
    task('T-3', 'blocked', { sprint: 'Later', waiting_on: 'user' }),
    task('T-4', 'in_review'),
    task('T-5', 'done', { sprint: 'Now', ...done }),
    task('T-6', 'done', done),
  ];
  const sprints = [
    { id: 'Now', state: 'active' },
    { id: 'Later', state: 'planned' },
  ];
  const ids = (board) =>
    Object.fromEntries(Object.entries(board).map(([column, c]) => [column, [c.count, c.cards.map((x) => x.id)]]));
  withProject({ meta: { sprints }, tasks }, (root) => {
    const s = summary(root);
    assert.deepEqual(ids(s.board_active_sprint), {
      todo: [1, ['T-1']],
      active: [1, ['T-2']],
      review: [0, []],
      blocked: [0, []],
      done: [1, ['T-5']],
    });
    // The whole board stays as it was: every sprint and the backlog
    assert.deepEqual(ids(s.board).blocked, [1, ['T-3']]);
    assert.deepEqual(ids(s.board).review, [1, ['T-4']]);
    assert.equal(s.board.done.count, 2);
  });
  withProject({ meta: { sprints: [{ id: 'Now' }] }, tasks }, (root) => {
    assert.equal(summary(root).board_active_sprint, null);
  });
  withProject({ tasks }, (root) => {
    assert.equal(summary(root).board_active_sprint, null);
  });
});

test("a project's own priorities and statuses; a phase left from before sprints doesn't count", () => {
  const meta = {
    statuses: [
      { id: 'backlog', kind: 'todo' },
      { id: 'doing', kind: 'active' },
      { id: 'shipped', kind: 'done' },
    ],
    priorities: [{ id: 'must' }, { id: 'should' }, { id: 'could' }],
  };
  const tasks = [
    task('T-1', 'backlog', { phase: 1, priority: 'must' }),
    task('T-9', 'backlog', { phase: 0, priority: 'could' }),
    task('T-5', 'backlog', { phase: 0 }),
    task('T-7', 'doing', { phase: 0, priority: 'must' }),
  ];
  withProject({ meta, tasks }, (root) => {
    const s = summary(root);
    assert.deepEqual(
      s.next.map((t) => t.id),
      ['T-1', 'T-5'],
    );
    assert.equal(atmNext(root), 'T-1');
  });
});
