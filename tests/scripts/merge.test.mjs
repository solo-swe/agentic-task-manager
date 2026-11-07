// The tasks.json merge driver, with real `git merge`s: the experiments from docs/parallel-agents-design.md §2
// (Evidence), now with the driver registered by `atm db migrate`/`atm db init`.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { describe, test } from 'node:test';
import { git, makeProject, removeDir, runAtm, tempDir } from './helpers.mjs';

const FILE = '.agentic-task-manager/tasks.json';
const id = (n) => `T-${String(n).padStart(3, '0')}`;
const BASE = {
  meta: { project: 'Merge' },
  tasks: Array.from({ length: 40 }, (_, i) => ({
    id: id(i + 1),
    name: `Task ${i + 1}`,
    phase: Math.floor(i / 10),
    type: 'feature',
    description: `Do thing ${i + 1}.`,
    status: i < 10 ? 'done' : 'todo',
    dependencies: i ? [id(i)] : [],
    created_at: '2026-04-01T00:00:00Z',
    updated_at: '2026-04-01T00:00:00Z',
  })),
};

const read = (root) => JSON.parse(fs.readFileSync(path.join(root, FILE), 'utf8'));
const write = (root, data) => fs.writeFileSync(path.join(root, FILE), JSON.stringify(data, null, 2) + '\n');
const find = (data, taskId) => data.tasks.find((t) => t.id === taskId);

/** A repo on `main` with the base task list and the merge driver registered (as any atm skill would leave it). */
function repo() {
  const root = makeProject({ commit: true, files: { [FILE]: BASE } });
  assert.equal(runAtm(['db', 'init'], root).code, 0);
  git(['commit', '-q', '-am', 'atm init'], root);
  return root;
}

/** Change tasks.json on a new branch `other` and on `main`, then merge `other` into `main`. */
function mergeBranches(root, onOther, onMain) {
  git(['switch', '-q', '-c', 'other'], root);
  const theirs = read(root);
  onOther(theirs);
  write(root, theirs);
  git(['commit', '-q', '-am', 'other'], root);
  git(['switch', '-q', 'main'], root);
  const ours = read(root);
  onMain(ours);
  write(root, ours);
  git(['commit', '-q', '-am', 'main'], root);
  const r = spawnSync('git', ['merge', '--no-edit', 'other'], { cwd: root, encoding: 'utf8' });
  return { clean: r.status === 0, output: r.stdout + r.stderr, merged: read(root) };
}

function withRepo(fn) {
  const root = repo();
  try {
    fn(root);
  } finally {
    removeDir(root);
  }
}

const progress = (task, status, at, extra = {}) => Object.assign(task, { status, updated_at: at, ...extra });

describe('merge driver (real git merges)', () => {
  test('atm db init registers the driver and .gitattributes; it is idempotent', () => {
    withRepo((root) => {
      assert.match(
        git(['config', '--get', 'merge.atm.driver'], root),
        // this atm, by its full path, as it was run
        /[/\\]atm(\.exe)?' db merge-driver %O %A %B$/,
      );
      assert.equal(
        fs.readFileSync(path.join(root, '.agentic-task-manager/.gitattributes'), 'utf8'),
        "# 'atm' is defined in each clone's git config (merge.atm.driver), registered by the atm plugin on every run.\n# Without it, git merges tasks.json line by line as usual.\ntasks.json merge=atm\n",
      );
      assert.match(runAtm(['db', 'init'], root).stdout, /already set/);
      // init also gave the project an id (for the dashboard's saved view), and keeps it
      const projectId = read(root).meta.id;
      assert.match(projectId, /^[0-9a-f]{8}$/);
      assert.doesNotMatch(runAtm(['db', 'init'], root).stdout, /Gave this project an id/);
      assert.equal(read(root).meta.id, projectId);
    });
  });

  test('status changes to adjacent tasks merge cleanly', () => {
    withRepo((root) => {
      const r = mergeBranches(
        root,
        (d) => progress(find(d, 'T-030'), 'in_progress', '2026-05-01T10:00:00Z'),
        (d) => progress(find(d, 'T-031'), 'in_progress', '2026-05-01T10:01:00Z'),
      );
      assert.ok(r.clean, r.output);
      assert.equal(find(r.merged, 'T-030').status, 'in_progress');
      assert.equal(find(r.merged, 'T-031').status, 'in_progress');
    });
  });

  test('finishing adjacent tasks (new timestamp fields on both sides) merges cleanly', () => {
    withRepo((root) => {
      const r = mergeBranches(
        root,
        (d) => progress(find(d, 'T-011'), 'done', '2026-05-01T11:00:00Z', { started_at: 'a', completed_at: 'b' }),
        (d) => progress(find(d, 'T-012'), 'done', '2026-05-01T11:05:00Z', { started_at: 'c', completed_at: 'd' }),
      );
      assert.ok(r.clean, r.output);
      assert.equal(find(r.merged, 'T-011').completed_at, 'b');
      assert.equal(find(r.merged, 'T-012').completed_at, 'd');
    });
  });

  test('the same task claimed on both branches: clean, in progress, the newer claim’s timestamps', () => {
    withRepo((root) => {
      const r = mergeBranches(
        root,
        (d) =>
          progress(find(d, 'T-020'), 'in_progress', '2026-05-01T10:05:00Z', { started_at: '2026-05-01T10:05:00Z' }),
        (d) =>
          progress(find(d, 'T-020'), 'in_progress', '2026-05-01T10:00:00Z', { started_at: '2026-05-01T10:00:00Z' }),
      );
      assert.ok(r.clean, r.output);
      const t = find(r.merged, 'T-020');
      assert.equal(t.status, 'in_progress');
      assert.equal(t.started_at, '2026-05-01T10:05:00Z');
    });
  });

  test('done beats in progress, whichever side it is on', () => {
    withRepo((root) => {
      const r = mergeBranches(
        root,
        (d) => progress(find(d, 'T-015'), 'done', '2026-05-01T09:00:00Z', { completed_at: '2026-05-01T09:00:00Z' }),
        (d) => progress(find(d, 'T-015'), 'in_progress', '2026-05-01T12:00:00Z'),
      );
      assert.ok(r.clean, r.output);
      assert.equal(find(r.merged, 'T-015').status, 'done');
      assert.equal(find(r.merged, 'T-015').completed_at, '2026-05-01T09:00:00Z');
    });
  });

  test('a definition change on one side and a status change on the other both survive', () => {
    withRepo((root) => {
      const r = mergeBranches(
        root,
        (d) => (find(d, 'T-025').description = 'Clarified.'),
        (d) => progress(find(d, 'T-025'), 'in_progress', '2026-05-01T10:00:00Z'),
      );
      assert.ok(r.clean, r.output);
      assert.equal(find(r.merged, 'T-025').description, 'Clarified.');
      assert.equal(find(r.merged, 'T-025').status, 'in_progress');
    });
  });

  test('both branches add tasks with the same IDs: both kept, theirs renumbered, their dependencies follow', () => {
    withRepo((root) => {
      const add = (d, name, deps = []) =>
        d.tasks.push({ id: id(d.tasks.length + 1), name, status: 'todo', dependencies: deps });
      const r = mergeBranches(
        root,
        (d) => {
          add(d, 'theirs A');
          add(d, 'theirs B', ['T-041']);
        },
        (d) => add(d, 'ours A'),
      );
      assert.ok(r.clean, r.output);
      const tail = r.merged.tasks.slice(40).map((t) => [t.id, t.name, t.dependencies]);
      assert.deepEqual(tail, [
        ['T-041', 'ours A', []],
        ['T-043', 'theirs A', []],
        ['T-042', 'theirs B', ['T-043']],
      ]);
    });
  });

  test('the same task’s description edited differently: the merge stops and says where', () => {
    withRepo((root) => {
      const r = mergeBranches(
        root,
        (d) => (find(d, 'T-033').description = 'Their wording.'),
        (d) => (find(d, 'T-033').description = 'Our wording.'),
      );
      assert.equal(r.clean, false);
      assert.match(r.output, /T-033: both branches changed description/);
      // The file is left valid JSON with our version, for a human to resolve
      assert.equal(find(r.merged, 'T-033').description, 'Our wording.');
      assert.match(git(['status', '--porcelain'], root), /^UU \.agentic-task-manager\/tasks\.json/m);
    });
  });

  test('a task deleted on one side and untouched on the other is deleted', () => {
    withRepo((root) => {
      const r = mergeBranches(
        root,
        (d) => (d.tasks = d.tasks.filter((t) => t.id !== 'T-040')),
        (d) => progress(find(d, 'T-011'), 'in_progress', '2026-05-01T10:00:00Z'),
      );
      assert.ok(r.clean, r.output);
      assert.equal(find(r.merged, 'T-040'), undefined);
    });
  });
});

// meta.sprints merges item by item, like tasks (docs/sprints-and-releases-design.md SR-2.4, SR-8.5)
describe('merge driver: sprints', () => {
  const sprint = (d, name) => d.meta.sprints.find((s) => s.id === name);
  /** A repo whose committed base has two sprints, Alpha (active) and Beta (planned), and T-011 in Alpha. */
  function withSprints(fn) {
    withRepo((root) => {
      const d = read(root);
      d.meta.sprints = [
        { id: 'Alpha', state: 'active', goal: 'First', start: '2026-05-01T09:00:00Z' },
        { id: 'Beta', state: 'planned' },
      ];
      Object.assign(find(d, 'T-011'), {
        sprint: 'Alpha',
        sprint_history: [{ sprint: 'Alpha', from: '2026-05-01T09:00:00Z' }],
      });
      write(root, d);
      git(['commit', '-q', '-am', 'sprints'], root);
      fn(root);
    });
  }

  test('a sprint added on one branch and another closed on the other: both kept', () => {
    withSprints((root) => {
      const r = mergeBranches(
        root,
        (d) => d.meta.sprints.push({ id: 'Gamma', state: 'planned' }),
        (d) => Object.assign(sprint(d, 'Alpha'), { state: 'closed', end: '2026-05-03T09:00:00Z' }),
      );
      assert.ok(r.clean, r.output);
      assert.deepEqual(
        r.merged.meta.sprints.map((s) => [s.id, s.state]),
        [
          ['Alpha', 'closed'],
          ['Beta', 'planned'],
          ['Gamma', 'planned'],
        ],
      );
    });
  });

  test('a sprint deleted on one branch stays deleted; changed on the other, it comes back as a conflict (T-339)', () => {
    withSprints((root) => {
      const deleteBeta = (d) => (d.meta.sprints = d.meta.sprints.filter((s) => s.id !== 'Beta'));
      // Deleted here, the other branch added a sprint without touching Beta
      const r = mergeBranches(root, (d) => d.meta.sprints.push({ id: 'Gamma', state: 'planned' }), deleteBeta);
      assert.ok(r.clean, r.output);
      assert.deepEqual(
        r.merged.meta.sprints.map((s) => s.id),
        ['Alpha', 'Gamma'],
      );
    });
    withSprints((root) => {
      const deleteBeta = (d) => (d.meta.sprints = d.meta.sprints.filter((s) => s.id !== 'Beta'));
      const r = mergeBranches(root, (d) => (sprint(d, 'Beta').goal = 'Kept after all'), deleteBeta);
      assert.ok(!r.clean);
      assert.match(r.output, /meta\.sprints: 'Beta' deleted on this branch, changed on the other/);
    });
  });

  test('one branch closes a sprint, the other changes its goal: closed, with the new goal', () => {
    withSprints((root) => {
      const r = mergeBranches(
        root,
        (d) => (sprint(d, 'Alpha').goal = 'Sharper'),
        (d) =>
          Object.assign(sprint(d, 'Alpha'), { state: 'closed', end: '2026-05-03T09:00:00Z', report: { done: [] } }),
      );
      assert.ok(r.clean, r.output);
      assert.deepEqual(sprint(r.merged, 'Alpha'), {
        id: 'Alpha',
        state: 'closed',
        goal: 'Sharper',
        start: '2026-05-01T09:00:00Z',
        end: '2026-05-03T09:00:00Z',
        report: { done: [] },
      });
    });
  });

  test('both branches change a sprint’s state: the further one wins, with its fields', () => {
    withSprints((root) => {
      const r = mergeBranches(
        root,
        (d) => Object.assign(sprint(d, 'Beta'), { state: 'closed', end: '2026-05-04T09:00:00Z' }),
        (d) => Object.assign(sprint(d, 'Beta'), { state: 'active', start: '2026-05-02T09:00:00Z' }),
      );
      assert.ok(r.clean, r.output);
      assert.equal(sprint(r.merged, 'Beta').state, 'closed');
      assert.equal(sprint(r.merged, 'Beta').end, '2026-05-04T09:00:00Z');
    });
  });

  test('both branches change a sprint’s goal differently: the merge stops and says where', () => {
    withSprints((root) => {
      const r = mergeBranches(
        root,
        (d) => (sprint(d, 'Alpha').goal = 'Theirs'),
        (d) => (sprint(d, 'Alpha').goal = 'Ours'),
      );
      assert.equal(r.clean, false);
      assert.match(r.output, /meta\.sprints: both branches changed 'Alpha' \(goal\)/);
      assert.equal(sprint(r.merged, 'Alpha').goal, 'Ours');
    });
  });

  test('two different sprints added with the same name (in any case) are a conflict', () => {
    withSprints((root) => {
      const r = mergeBranches(
        root,
        (d) => d.meta.sprints.push({ id: 'gamma', state: 'planned', goal: 'Theirs' }),
        (d) => d.meta.sprints.push({ id: 'Gamma', state: 'planned', goal: 'Ours' }),
      );
      assert.equal(r.clean, false);
      assert.match(r.output, /meta\.sprints: 'Gamma' was added on both branches, differently/);
    });
  });

  test('a task moved to another sprint on one branch and progressed on the other: moved, its history closed', () => {
    withSprints((root) => {
      const r = mergeBranches(
        root,
        (d) => progress(find(d, 'T-011'), 'in_progress', '2026-05-02T10:00:00Z'),
        (d) =>
          Object.assign(find(d, 'T-011'), {
            sprint: 'Beta',
            sprint_history: [
              { sprint: 'Alpha', from: '2026-05-01T09:00:00Z', to: '2026-05-02T09:00:00Z' },
              { sprint: 'Beta', from: '2026-05-02T09:00:00Z' },
            ],
          }),
      );
      assert.ok(r.clean, r.output);
      const t = find(r.merged, 'T-011');
      assert.equal(t.sprint, 'Beta');
      assert.equal(t.status, 'in_progress');
      assert.deepEqual(t.sprint_history, [
        { sprint: 'Alpha', from: '2026-05-01T09:00:00Z', to: '2026-05-02T09:00:00Z' },
        { sprint: 'Beta', from: '2026-05-02T09:00:00Z' },
      ]);
    });
  });

  test('a branch migrated to sprints merged with one that still has phases: sprints win, phases go', () => {
    withRepo((root) => {
      const r = mergeBranches(
        root,
        (d) => {
          d.meta.phases = [{ id: 1, name: 'Renamed on the old branch' }];
          find(d, 'T-015').phase = 3;
        },
        (d) => {
          d.meta.sprints = [{ id: 'Phase 1', state: 'planned' }];
          for (const t of d.tasks) {
            if (t.phase === 1) Object.assign(t, { sprint: 'Phase 1' });
            delete t.phase;
          }
        },
      );
      assert.ok(r.clean, r.output);
      assert.equal(r.merged.meta.phases, undefined);
      const t = find(r.merged, 'T-015');
      assert.equal(t.sprint, 'Phase 1');
      assert.equal(t.phase, undefined);
    });
  });
});

describe('merge-driver command', () => {
  const files = (base, ours, theirs) => {
    const dir = tempDir();
    const paths = ['base', 'ours', 'theirs'].map((n) => path.join(dir, n));
    [base, ours, theirs].forEach((c, i) => fs.writeFileSync(paths[i], c));
    return { dir, paths };
  };

  test('a side that is not valid JSON is a conflict, and ours is left alone', () => {
    const { dir, paths } = files('{"tasks": []}', '{"tasks": [', '{"tasks": []}');
    try {
      const r = runAtm(['db', 'merge-driver', ...paths], dir);
      assert.equal(r.code, 1);
      assert.match(r.stderr, /not valid JSON/);
      assert.equal(fs.readFileSync(paths[1], 'utf8'), '{"tasks": [');
    } finally {
      removeDir(dir);
    }
  });

  test('a file added on both branches (empty base) merges by task', () => {
    const t = (taskId, status) => ({ id: taskId, status, dependencies: [] });
    const { dir, paths } = files(
      '',
      JSON.stringify({ tasks: [t('T-1', 'todo')] }),
      JSON.stringify({ tasks: [t('T-1', 'todo'), t('T-2', 'todo')] }),
    );
    try {
      assert.equal(runAtm(['db', 'merge-driver', ...paths], dir).code, 0);
      assert.deepEqual(JSON.parse(fs.readFileSync(paths[1], 'utf8')).tasks, [t('T-1', 'todo'), t('T-2', 'todo')]);
    } finally {
      removeDir(dir);
    }
  });
});
