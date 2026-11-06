// atm2 (the Typer-based CLI, T-412) against atm: each step runs an `atm` command on one project and its atm2
// equivalent on an identical one, and both must give the same exit code, stdout, stderr and tasks.json. atm2 needs
// Typer (uv's dev environment); where it can't run, these tests are skipped.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { describe, test } from 'node:test';
import { REPO, baseEnv, makeProject, removeDir, runAtm } from './helpers.mjs';

const ATM2 = path.join(REPO, 'atm-cli/atm2');
const ENV = { ATM_AGENT: 'alice', ATM_NOW: '2026-11-05T10:00:00Z' };
const available = spawnSync(ATM2, ['--help'], { encoding: 'utf8', env: baseEnv() }).status === 0;

function runAtm2(args, cwd) {
  const result = spawnSync(ATM2, args, { cwd, encoding: 'utf8', env: { ...baseEnv(), ...ENV } });
  if (result.error) throw result.error;
  return { code: result.status, stdout: result.stdout, stderr: result.stderr };
}

const tasksOf = (root) => fs.readFileSync(path.join(root, '.agentic-task-manager/tasks.json'), 'utf8');

const SETUP = {
  files: {
    '.agentic-task-manager/tasks.json': {
      tasks: [
        { id: 'T-1', name: 'First', type: 'feature', status: 'todo', description: 'Do the first thing' },
        { id: 'T-2', name: 'Second', type: 'bug', status: 'todo', dependencies: ['T-1'], labels: ['ui'] },
        { id: 'T-3', name: 'Third', type: 'chore', status: 'todo' },
      ],
    },
  },
  commit: true,
};

/** Run each [atm args, atm2 args] step on both projects, in order, comparing every result. */
function parity(steps) {
  const one = makeProject(SETUP);
  const two = makeProject(SETUP);
  try {
    for (const [atmArgs, atm2Args] of steps) {
      const label = `atm ${atmArgs.join(' ')} / atm2 ${atm2Args.join(' ')}`;
      const expected = runAtm(atmArgs, one, ENV);
      const actual = runAtm2(atm2Args, two);
      assert.deepEqual(actual, expected, label);
      assert.equal(tasksOf(two), tasksOf(one), `${label}: tasks.json`);
    }
  } finally {
    removeDir(one);
    removeDir(two);
  }
}

describe('atm2 task group', { skip: !available && 'atm2 needs Typer (uv sync --dev)' }, () => {
  test('list, details and deps read the same', () => {
    parity([
      [['list'], ['task', 'list']],
      [
        ['list', '--status', 'todo', '--label', 'ui'],
        ['task', 'list', '--status', 'todo', '--label', 'ui'],
      ],
      [
        ['list', '--json'],
        ['-j', 'task', 'list'],
      ],
      [
        ['list', '--ready'],
        ['task', 'list', '--ready'],
      ],
      [
        ['show', 'T-1', 'T-2'],
        ['task', 'details', 'T-1', 'T-2'],
      ],
      [
        ['show', 'T-2', '--json'],
        ['-j', 'task', 'details', 'T-2'],
      ],
      [
        ['deps', 'T-2'],
        ['task', 'deps', 'T-2'],
      ],
    ]);
  });

  test('errors are reported as atm reports them', () => {
    parity([
      [
        ['show', 'T-9'],
        ['task', 'details', 'T-9'],
      ],
      [
        ['show', 'T-9', '--json'],
        ['-j', 'task', 'details', 'T-9'],
      ],
      [
        ['deps', 'T-9'],
        ['task', 'deps', 'T-9'],
      ],
    ]);
  });

  test('add, edit and note change the same', () => {
    parity([
      [
        ['add', '--name', 'Fourth', '--type', 'feature', '--label', 'api', '--dep', 'T-1'],
        ['task', 'add', '--name', 'Fourth', '--type', 'feature', '--label', 'api', '--dep', 'T-1'],
      ],
      [
        ['add', '--name', 'Fifth', '--type', 'nope'],
        ['task', 'add', '--name', 'Fifth', '--type', 'nope'],
      ],
      [
        ['edit', 'T-3', '--name', 'Third, renamed', '--add-label', 'x', '--priority', 'P1'],
        ['task', 'edit', 'T-3', '--name', 'Third, renamed', '--add-label', 'x', '--priority', 'P1'],
      ],
      [
        ['edit', 'T-3', '--json'],
        ['-j', 'task', 'edit', 'T-3'],
      ],
      [
        ['note', 'T-1', 'a note'],
        ['task', 'note', 'T-1', 'a note'],
      ],
      [
        ['note', 'T-1', 'another', '--json'],
        ['-j', 'task', 'note', 'T-1', 'another'],
      ],
    ]);
  });

  test('state moves a task through review the way the status commands do', () => {
    parity([
      [
        ['start', 'T-1'],
        ['task', 'state', 'T-1', 'in_progress'],
      ],
      [
        ['review', 'T-1', '--note', 'look'],
        ['task', 'state', 'T-1', 'in_review', '--note', 'look'],
      ],
      [
        ['reject', 'T-1', '--reason', 'redo'],
        ['task', 'state', 'T-1', 'todo', '--reason', 'redo'],
      ],
      [
        ['start', 'T-1'],
        ['task', 'state', 'T-1', 'in_progress'],
      ],
      [
        ['review', 'T-1'],
        ['task', 'state', 'T-1', 'in_review'],
      ],
      [
        ['request-changes', 'T-1', '--reason', 'tweak'],
        ['task', 'state', 'T-1', 'changes_requested', '--reason', 'tweak'],
      ],
      [
        ['start', 'T-1'],
        ['task', 'state', 'T-1', 'in_progress'],
      ],
      [
        ['done', 'T-1', '--note', 'finished', '--json'],
        ['-j', 'task', 'state', 'T-1', 'done', '--note', 'finished'],
      ],
      [
        ['reopen', 'T-1', '--reason', 'not yet'],
        ['task', 'state', 'T-1', 'todo', '--reason', 'not yet'],
      ],
    ]);
  });

  test('state: back to todo from in progress (fail, release) and from blocked (unblock), and skip', () => {
    parity([
      [
        ['start', 'T-3'],
        ['task', 'state', 'T-3', 'in_progress'],
      ],
      [
        ['fail', 'T-3', '--reason', 'broke'],
        ['task', 'state', 'T-3', 'todo', '--reason', 'broke'],
      ],
      [
        ['start', 'T-3'],
        ['task', 'state', 'T-3', 'in_progress'],
      ],
      [
        ['release', 'T-3', '--note', 'handoff'],
        ['task', 'state', 'T-3', 'todo', '--note', 'handoff'],
      ],
      [
        ['block', 'T-3', '--on', 'user', '--question', 'which?'],
        ['task', 'state', 'T-3', 'blocked', '--on', 'user', '--question', 'which?'],
      ],
      [
        ['unblock', 'T-3', '--answer', 'this'],
        ['task', 'state', 'T-3', 'todo', '--answer', 'this'],
      ],
      [
        ['skip', 'T-3', '--reason', 'not needed'],
        ['task', 'state', 'T-3', 'skipped', '--reason', 'not needed'],
      ],
      [
        ['start', 'T-2'],
        ['task', 'state', 'T-2', 'in_progress'],
      ],
    ]);
  });
});

describe('atm2 task state, its own checks', { skip: !available && 'atm2 needs Typer (uv sync --dev)' }, () => {
  test('refuses options that are not for the transition, and asks for the ones it needs', () => {
    const root = makeProject(SETUP);
    try {
      const wrong = runAtm2(['task', 'state', 'T-1', 'in_progress', '--commit', 'abc'], root);
      assert.equal(wrong.code, 2);
      assert.match(wrong.stderr, /--commit: not for in_progress \(start\)/);
      const missing = runAtm2(['task', 'state', 'T-1', 'skipped'], root);
      assert.equal(missing.code, 2);
      assert.match(missing.stderr, /skipped \(skip\) needs --reason/);
      const already = runAtm2(['task', 'state', 'T-1', 'todo'], root);
      assert.equal(already.code, 2);
      assert.match(already.stderr, /T-1 is already todo/);
    } finally {
      removeDir(root);
    }
  });

  test('--dry-run says which command it would run and changes nothing', () => {
    const root = makeProject(SETUP);
    try {
      const before = tasksOf(root);
      const result = runAtm2(['-n', 'task', 'state', 'T-1', 'in_progress'], root);
      assert.equal(result.code, 0);
      assert.equal(result.stdout, '[dry-run] would start T-1\n');
      assert.equal(tasksOf(root), before);
    } finally {
      removeDir(root);
    }
  });
});
