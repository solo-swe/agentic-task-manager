// The command line itself (T-425): the tree's help and its order, hidden commands, the options that go anywhere,
// usage errors, shell completion and `task state`'s own checks. What each command does is in scenarios/ and the other
// tests.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { describe, test } from 'node:test';
import { makeProject, removeDir, runAtm } from './helpers.mjs';

const ENV = { ATM_AGENT: 'alice', ATM_NOW: '2026-11-05T10:00:00Z', COLUMNS: '120' };
const atm = (args, cwd, env = {}) => runAtm(args, cwd, { ...ENV, ...env });
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

/** Run `body(root)` on a fresh project made from `setup`. */
function inProject(setup, body) {
  const root = makeProject(setup);
  try {
    body(root);
  } finally {
    removeDir(root);
  }
}

describe('the command tree', () => {
  test('--help lists the everyday commands first; maintenance, then help and version, after them', () => {
    inProject(SETUP, (root) => {
      const help = atm(['--help'], root);
      assert.equal(help.code, 0, help.stderr);
      // The command names, in the order the Commands section lists them (plain Click help: no rich panels)
      const listed = help.stdout
        .split('Commands:\n')[1]
        .split('\n')
        .map((line) => /^ {2}(\S+)/.exec(line)?.[1])
        .filter(Boolean);
      assert.deepEqual(listed, [
        ...['task', 'loop', 'claim', 'vcs', 'release', 'component', 'project', 'sprint'],
        ...['db', 'doctor', 'service'],
        ...['help', 'version'],
      ]);
    });
  });

  test('db merge-driver, project summary and _lock-test work but are hidden', () => {
    inProject(SETUP, (root) => {
      const db = atm(['db', '--help'], root);
      assert.equal(db.code, 0, db.stderr);
      assert.doesNotMatch(db.stdout, /merge-driver/);
      assert.match(db.stdout, /init/);
      const project = atm(['project', '--help'], root);
      assert.doesNotMatch(project.stdout, /summary/);
      assert.match(project.stdout, /config/);
      assert.doesNotMatch(atm(['--help'], root).stdout, /_lock-test/);
      assert.equal(atm(['project', 'summary'], root).code, 0);
      assert.equal(atm(['_lock-test', '0'], root).code, 0);
    });
  });

  test('help prints the same list as --help', () => {
    inProject(SETUP, (root) => {
      const help = atm(['help'], root);
      assert.equal(help.code, 0, help.stderr);
      assert.equal(help.stdout, atm(['--help'], root).stdout);
    });
  });

  test('the old flat commands are gone, not aliases', () => {
    inProject(SETUP, (root) => {
      for (const old of ['start', 'done', 'show', 'list', 'sprints', 'check', 'serve', 'commit']) {
        const r = atm([old, 'T-1'], root);
        assert.equal(r.code, 2, `${old}: ${r.stdout}`);
        assert.match(r.stderr, new RegExp(`^atm: No such command '${old}'`));
      }
      assert.match(tasksOf(root), /"status": "todo"/);
    });
  });
});

describe('options that go anywhere, and errors', () => {
  test('--json and -j work before the command, after it, or among its options', () => {
    inProject(SETUP, (root) => {
      const before = atm(['--json', 'task', 'deps', 'T-2'], root);
      const after = atm(['task', 'deps', 'T-2', '--json'], root);
      const short = atm(['task', '-j', 'deps', 'T-2'], root);
      assert.equal(before.code, 0, before.stderr);
      assert.deepEqual(JSON.parse(before.stdout).waits_for, [{ id: 'T-1', status: 'todo', name: 'First' }]);
      assert.equal(after.stdout, before.stdout);
      assert.equal(short.stdout, before.stdout);
    });
  });

  test('--dry-run (-n) anywhere changes nothing', () => {
    inProject(SETUP, (root) => {
      const before = tasksOf(root);
      const result = atm(['task', 'state', 'T-1', 'in_progress', '-n'], root);
      assert.equal(result.code, 0, result.stderr);
      assert.equal(result.stdout, '[dry-run] would start T-1\n');
      assert.equal(tasksOf(root), before);
    });
  });

  test('--dir first runs as if started in that folder; a missing one is a usage error', () => {
    inProject(SETUP, (root) => {
      const elsewhere = makeProject({});
      try {
        const r = atm(['--dir', root, 'task', 'deps', 'T-2', '--json'], elsewhere);
        assert.equal(r.code, 0, r.stderr);
        assert.equal(JSON.parse(r.stdout).id, 'T-2');
        const missing = atm(['--dir', path.join(root, 'nope'), 'project', 'status'], elsewhere);
        assert.equal(missing.code, 2);
        assert.match(missing.stderr, /^atm: --dir: no folder/);
      } finally {
        removeDir(elsewhere);
      }
    });
  });

  test('a usage error is `atm: <message>` and the usage line, exit code 2; with --json, JSON', () => {
    inProject(SETUP, (root) => {
      const text = atm(['task', 'state'], root);
      assert.equal(text.code, 2);
      assert.equal(
        text.stderr,
        "atm: Missing argument 'TASK_ID'.\nUsage: atm task state [OPTIONS] TASK_ID NEXT_STATE\n",
      );
      const json = atm(['task', 'state', '--json'], root);
      assert.equal(json.code, 2);
      assert.deepEqual(JSON.parse(json.stderr), { error: "Missing argument 'TASK_ID'.", code: 2 });
    });
  });

  test('atm runs no command it is given: there is no loop run, and no command takes one (T-437)', () => {
    inProject(SETUP, (root) => {
      atm(['task', 'state', 'T-1', 'in_progress'], root);
      const marker = path.join(root, 'ran');
      const r = atm(['loop', 'run', 'T-1', '--', 'touch', marker], root);
      assert.equal(r.code, 2);
      assert.match(r.stderr, /^atm: No such command 'run'/);
      assert.equal(fs.existsSync(marker), false);
      const loop = atm(['loop', '--help'], root);
      assert.deepEqual(
        loop.stdout
          .split('Commands:\n')[1]
          .split('\n')
          .map((line) => /^ {2}(\S+)/.exec(line)?.[1])
          .filter(Boolean),
        ['next'],
      );
    });
  });
});

/** What the shell would offer after `line` (fish's format: one `value<TAB>help` per line). */
function complete(line, cwd) {
  const env = { _ATM_COMPLETE: 'complete_fish', _TYPER_COMPLETE_FISH_ACTION: 'get-args', _TYPER_COMPLETE_ARGS: line };
  const result = atm([], cwd, env);
  assert.equal(result.code, 0, result.stderr);
  return result.stdout.split('\n').filter(Boolean);
}

describe('shell completion', () => {
  const SPRINTS = {
    ...SETUP,
    files: {
      '.agentic-task-manager/tasks.json': {
        meta: {
          sprints: [
            { id: 'First', state: 'active', goal: 'The basics' },
            { id: 'Old', state: 'closed' },
          ],
          statuses: [
            { id: 'todo', label: 'To Do', kind: 'todo' },
            { id: 'doing', label: 'Doing', kind: 'active' },
            { id: 'awaiting_user', label: 'Awaiting you', kind: 'review' },
            { id: 'shipped', label: 'Shipped', kind: 'done' },
          ],
        },
        tasks: SETUP.files['.agentic-task-manager/tasks.json'].tasks,
      },
    },
  };

  test('task IDs with their names, sprints, statuses and the targets of task state', () => {
    inProject(SPRINTS, (root) => {
      assert.deepEqual(complete('atm task details T-', root), ['T-1\tFirst', 'T-2\tSecond', 'T-3\tThird']);
      assert.deepEqual(complete('atm task deps T-2', root), ['T-2\tSecond']);
      assert.deepEqual(complete('atm task list --sprint ', root), ['First\tThe basics']);
      assert.deepEqual(complete('atm task list --status a', root), ['awaiting_user\tAwaiting you']);
      assert.deepEqual(complete('atm task list --priority P', root), [
        'P0\tCritical',
        'P1\tHigh',
        'P2\tMedium',
        'P3\tLow',
      ]);
      assert.deepEqual(complete('atm task state T-1 in_', root), ['in_progress\tstart', 'in_review\treview']);
      assert.deepEqual(complete('atm sprint start ', root), ['First\tThe basics']);
    });
  });

  test('outside a project, or with a task file atm can’t read, it completes nothing', () => {
    const empty = makeProject({});
    const broken = makeProject({ files: { '.agentic-task-manager/tasks.json': '{ not json' } });
    try {
      assert.deepEqual(complete('atm task details T-', empty), []);
      assert.deepEqual(complete('atm task list --sprint ', broken), []);
    } finally {
      removeDir(empty);
      removeDir(broken);
    }
  });
});

describe('task state, its own checks', () => {
  test('refuses options that are not for the transition, and asks for the ones it needs', () => {
    inProject(SETUP, (root) => {
      const wrong = atm(['task', 'state', 'T-1', 'in_progress', '--commit', 'abc'], root);
      assert.equal(wrong.code, 2);
      assert.match(wrong.stderr, /--commit: not for in_progress \(start\)/);
      const missing = atm(['task', 'state', 'T-1', 'skipped'], root);
      assert.equal(missing.code, 2);
      assert.match(missing.stderr, /skipped \(skip\) needs --reason/);
      const already = atm(['task', 'state', 'T-1', 'todo'], root);
      assert.equal(already.code, 2);
      assert.match(already.stderr, /T-1 is already todo/);
      const unknown = atm(['task', 'state', 'T-1', 'finished'], root);
      assert.equal(unknown.code, 2);
      assert.match(unknown.stderr, /^atm: unknown state 'finished'/);
    });
  });

  test('back to todo from in progress: --reason is a failure, otherwise a release (with --note, a handoff)', () => {
    inProject(SETUP, (root) => {
      atm(['task', 'state', 'T-1', 'in_progress'], root);
      atm(['task', 'state', 'T-1', 'todo', '--reason', 'tests fail'], root);
      atm(['task', 'state', 'T-1', 'in_progress'], root);
      atm(['task', 'state', 'T-1', 'todo', '--note', 'half done'], root);
      const notes = JSON.parse(atm(['task', 'details', 'T-1', '--json'], root).stdout)[0].notes;
      assert.deepEqual(
        notes.map((n) => [n.kind, n.text]),
        [
          ['failure', 'tests fail'],
          ['handoff', 'half done'],
        ],
      );
    });
  });
});
