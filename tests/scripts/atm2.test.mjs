// atm2 (the Typer-based CLI, T-412) against atm: each step runs an `atm` command on one project and its atm2
// equivalent on an identical one, and both must give the same exit code, stdout, stderr and tasks.json. atm2 needs
// Typer (uv's dev environment, or atm-cli/atm2-requirements.txt installed with pip); where it can't run, these tests
// are skipped, unless ATM2_REQUIRED is set (CI installs Typer and sets it), when that is a failure.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { describe, test } from 'node:test';
import { REPO, baseEnv, makeProject, removeDir, runAtm } from './helpers.mjs';

const ATM2 = path.join(REPO, 'atm-cli/atm2');
const ENV = { ATM_AGENT: 'alice', ATM_NOW: '2026-11-05T10:00:00Z' };
const probe = spawnSync(ATM2, ['--help'], { encoding: 'utf8', env: baseEnv() });
const available = probe.status === 0;

test('atm2 runs where it is required (ATM2_REQUIRED, as CI sets it)', { skip: !process.env.ATM2_REQUIRED }, () => {
  assert.equal(probe.status, 0, `atm2 --help failed:\n${probe.stderr}`);
});

function runAtm2(args, cwd, env = ENV) {
  const result = spawnSync(ATM2, args, { cwd, encoding: 'utf8', env: { ...baseEnv(), ...env } });
  if (result.error) throw result.error;
  return { code: result.status, stdout: result.stdout, stderr: result.stderr };
}

/** Commit SHAs (full or short) as <sha>: the two projects' repositories make different commits of the same change. */
const unsha = (text) => text.replace(/\b[0-9a-f]{40}\b/g, '<sha>').replace(/\b[0-9a-f]{7}\b(?= )/g, '<sha>');

const tasksOf = (root) => unsha(fs.readFileSync(path.join(root, '.agentic-task-manager/tasks.json'), 'utf8'));

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

/** A result with its project's folder and its atm home written as <root> and <home>, so two projects compare
 * (claims name their worktree, the project registry its folder). */
function relative(result, root, home) {
  const names = [
    [fs.realpathSync(root), '<root>'],
    [root, '<root>'],
    [fs.realpathSync(home), '<home>'],
    [home, '<home>'],
  ];
  const strip = (text) => unsha(names.reduce((t, [from, to]) => t.split(from).join(to), text));
  return { ...result, stdout: strip(result.stdout), stderr: strip(result.stderr) };
}

/** Run each [atm args, atm2 args] step on two identical projects, each with its own atm home (its project registry),
 * in order, comparing every result. A step that is a function prepares both projects instead: it is called with each
 * project's folder (to write a file the next step commits, say). */
function parity(steps, setup = SETUP) {
  const one = makeProject(setup);
  const two = makeProject(setup);
  const homeOne = fs.mkdtempSync(path.join(os.tmpdir(), 'atm2-home-'));
  const homeTwo = fs.mkdtempSync(path.join(os.tmpdir(), 'atm2-home-'));
  try {
    for (const step of steps) {
      if (typeof step === 'function') {
        step(one);
        step(two);
        continue;
      }
      const [atmArgs, atm2Args] = step;
      const label = `atm ${atmArgs.join(' ')} / atm2 ${atm2Args.join(' ')}`;
      const expected = relative(runAtm(atmArgs, one, { ...ENV, ATM_HOME: homeOne }), one, homeOne);
      const actual = relative(runAtm2(atm2Args, two, { ...ENV, ATM_HOME: homeTwo }), two, homeTwo);
      assert.deepEqual(actual, expected, label);
      assert.equal(tasksOf(two), tasksOf(one), `${label}: tasks.json`);
    }
  } finally {
    for (const dir of [one, two, homeOne, homeTwo]) removeDir(dir);
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

describe('atm2 loop and claim groups', { skip: !available && 'atm2 needs Typer (uv sync --dev)' }, () => {
  test('next, next --claim, claims and heartbeat read and change the same', () => {
    parity([
      [['next'], ['loop', 'next']],
      [
        ['next', '--json', '--type', 'chore'],
        ['-j', 'loop', 'next', '--type', 'chore'],
      ],
      [
        ['next', '--tasks', 'T-2,T-3'],
        ['loop', 'next', '--tasks', 'T-2,T-3'],
      ],
      [['claims'], ['claim', 'list']],
      [
        ['next', '--claim', '--json'],
        ['-j', 'loop', 'next', '--claim'],
      ],
      [['claims'], ['claim', 'list']],
      [
        ['claims', '--json'],
        ['-j', 'claim', 'list'],
      ],
      [
        ['heartbeat', 'T-1'],
        ['claim', 'heartbeat', 'T-1'],
      ],
      [
        ['heartbeat', 'T-1', '--json'],
        ['-j', 'claim', 'heartbeat', 'T-1'],
      ],
      [['heartbeat'], ['claim', 'heartbeat']],
      [
        ['heartbeat', 'T-9'],
        ['claim', 'heartbeat', 'T-9'],
      ],
    ]);
  });

  test('next with nothing ready exits 3, the same', () => {
    parity([
      [
        ['next', '--tasks', 'T-2'],
        ['loop', 'next', '--tasks', 'T-2'],
      ],
      [
        ['next', '--json', '--tasks', 'T-2'],
        ['-j', 'loop', 'next', '--tasks', 'T-2'],
      ],
    ]);
  });

  test('run keeps the claim and passes the command’s exit code through', () => {
    parity([
      [
        ['start', 'T-1'],
        ['task', 'state', 'T-1', 'in_progress'],
      ],
      [
        ['run', 'T-1', '--', 'sh', '-c', 'echo running; exit 0'],
        ['loop', 'run', 'T-1', '--', 'sh', '-c', 'echo running; exit 0'],
      ],
      [
        ['run', 'T-1', '--', 'sh', '-c', 'exit 7'],
        ['loop', 'run', 'T-1', '--', 'sh', '-c', 'exit 7'],
      ],
      [
        ['run', 'T-3', '--', 'true'],
        ['loop', 'run', 'T-3', '--', 'true'],
      ],
    ]);
  });

  test('next refuses two selections at once', () => {
    const root = makeProject(SETUP);
    try {
      const result = runAtm2(['loop', 'next', '--all', '--backlog'], root);
      assert.equal(result.code, 2);
      assert.match(result.stderr, /--all and --backlog can't both be given/);
    } finally {
      removeDir(root);
    }
  });
});

const WITH_ID = {
  ...SETUP,
  files: {
    '.agentic-task-manager/tasks.json': {
      meta: { id: 'demo', project: 'Demo' },
      tasks: SETUP.files['.agentic-task-manager/tasks.json'].tasks,
    },
    'logo.svg': '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"/>\n',
  },
};

const write = (file, text) => (root) => {
  fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
  fs.writeFileSync(path.join(root, file), text);
};

describe('atm2 groups ported before T-420 (T-422)', { skip: !available && 'atm2 needs Typer (uv sync --dev)' }, () => {
  test('vcs: commit, record, pr, pushed and repository, the same', () => {
    parity([
      [
        ['start', 'T-1'],
        ['task', 'state', 'T-1', 'in_progress'],
      ],
      write('src/first.txt', 'one\n'),
      [
        ['commit', 'T-1', '--file', 'src/first.txt'],
        ['vcs', 'commit', 'T-1', '--file', 'src/first.txt'],
      ],
      write('src/second.txt', 'two\n'),
      [
        ['commit', 'T-1', '--file', 'src/second.txt', '--message', 'feat(T-1): the second file', '--json'],
        ['-j', 'vcs', 'commit', 'T-1', '--file', 'src/second.txt', '--message', 'feat(T-1): the second file'],
      ],
      [
        ['commit', 'T-1'],
        ['vcs', 'commit', 'T-1'],
      ],
      [
        ['commit', 'T-1', '--record', 'HEAD'],
        ['vcs', 'commit', 'T-1', '--record', 'HEAD'],
      ],
      [
        ['commit', 'T-1', '--record', 'HEAD', '--file', 'src/first.txt'],
        ['vcs', 'commit', 'T-1', '--record', 'HEAD', '--file', 'src/first.txt'],
      ],
      [
        ['done', 'T-1'],
        ['task', 'state', 'T-1', 'done'],
      ],
      [
        ['pr', 'T-1', '--number', '7', '--url', 'https://github.com/o/r/pull/7'],
        ['vcs', 'pr', 'T-1', '--number', '7', '--url', 'https://github.com/o/r/pull/7'],
      ],
      [
        ['merged', '--pr', '7', '--at', '2026-11-05T11:00:00Z'],
        ['vcs', 'pushed', '--pr', '7', '--at', '2026-11-05T11:00:00Z'],
      ],
      [
        ['merged', '--pr', '7', '--json'],
        ['-j', 'vcs', 'pushed', '--pr', '7'],
      ],
      [['repository'], ['vcs', 'repository']],
      [
        ['repository', 'set', 'https://github.com/o/r'],
        ['vcs', 'repository', 'https://github.com/o/r'],
      ],
      [
        ['repository', '--json'],
        ['-j', 'vcs', 'repository'],
      ],
    ]);
  });

  test('db: migrate, init, archive (and its dry run) and unarchive, the same', () => {
    parity(
      [
        [['migrate'], ['db', 'migrate']],
        [['init'], ['db', 'init']],
        [
          ['init', '--json'],
          ['-j', 'db', 'init'],
        ],
        [
          ['skip', 'T-3', '--reason', 'not needed'],
          ['task', 'state', 'T-3', 'skipped', '--reason', 'not needed'],
        ],
        [
          ['archive', '--keep-days', '0', '--dry-run'],
          ['-n', 'db', 'archive', '--keep-days', '0'],
        ],
        [
          ['archive', '--keep-days', '0'],
          ['db', 'archive', '--keep-days', '0'],
        ],
        [['archive'], ['db', 'archive']],
        [
          ['unarchive', 'T-3'],
          ['db', 'unarchive', 'T-3'],
        ],
        [
          ['unarchive', 'T-3'],
          ['db', 'unarchive', 'T-3'],
        ],
      ],
      // With an id already: `init` gives a project without one a random id, which two projects can't share
      WITH_ID,
    );
  });

  test('release: add, list, edit, notes and archive, the same', () => {
    parity([
      [['releases'], ['release', 'list']],
      [
        ['releases', 'add', '1.0', '--target', '2026-12-01'],
        ['release', 'add', '1.0', '--target', '2026-12-01'],
      ],
      [
        ['edit', 'T-1', '--release', '1.0'],
        ['task', 'edit', 'T-1', '--release', '1.0'],
      ],
      [
        ['releases', 'edit', '1.0', '--name', 'First'],
        ['release', 'edit', '1.0', '--name', 'First'],
      ],
      [['releases'], ['release', 'list']],
      [
        ['releases', '--json'],
        ['-j', 'release', 'list'],
      ],
      [
        ['releases', 'notes', '1.0'],
        ['release', 'notes', '1.0'],
      ],
      [
        ['releases', 'notes', '2.0'],
        ['release', 'notes', '2.0'],
      ],
      [
        ['releases', 'archive', '1.0'],
        ['release', 'archive', '1.0'],
      ],
      [
        ['releases', '--all', '--json'],
        ['-j', 'release', 'list', '--all'],
      ],
    ]);
  });

  test('component: add, edit and list, the same', () => {
    parity([
      [['components'], ['component', 'list']],
      [
        ['components', 'add', 'ui', '--path', 'src/**', '--label', 'UI'],
        ['component', 'add', 'ui', '--path', 'src/**', '--label', 'UI'],
      ],
      [
        ['components', 'edit', 'ui', '--review', 'batch'],
        ['component', 'edit', 'ui', '--review', 'batch'],
      ],
      [
        ['components', 'add', 'api', '--from-label', 'ui'],
        ['component', 'add', 'api', '--from-label', 'ui'],
      ],
      [['components'], ['component', 'list']],
      [
        ['components', '--json'],
        ['-j', 'component', 'list'],
      ],
    ]);
  });

  test('doctor and service status, the same', () => {
    parity([
      [['check'], ['doctor']],
      [
        ['check', '--files', '--json'],
        ['-j', 'doctor', '--files'],
      ],
      [
        ['serve', 'status'],
        ['service', 'status'],
      ],
    ]);
  });

  test('reads run under --dry-run: they change nothing', () => {
    const root = makeProject(SETUP);
    try {
      for (const args of [
        ['-n', 'release', 'list'],
        ['-n', 'component', 'list'],
        ['-n', 'claim', 'list'],
      ]) {
        const result = runAtm2(args, root);
        assert.equal(result.code, 0, result.stderr);
        assert.doesNotMatch(result.stdout, /\[dry-run\]/, args.join(' '));
      }
    } finally {
      removeDir(root);
    }
  });
});

describe('atm2 project group', { skip: !available && 'atm2 needs Typer (uv sync --dev)' }, () => {
  test('config, status and workflow read and change the same', () => {
    parity([
      [['paths'], ['project', 'config']],
      [
        ['paths', '--json'],
        ['-j', 'project', 'config'],
      ],
      [['status'], ['project', 'status']],
      [
        ['status', '--json'],
        ['-j', 'project', 'status'],
      ],
      [['workflow'], ['project', 'workflow']],
      [
        ['workflow', 'set', 'pull-request', '--base', 'main', '--verify', 'false', '--trailers', 'forbid'],
        ['project', 'workflow', 'set', 'pull-request', '--base', 'main', '--verify', 'false', '--trailers', 'forbid'],
      ],
      [['workflow'], ['project', 'workflow']],
      [
        ['workflow', '--json'],
        ['-j', 'project', 'workflow'],
      ],
      [
        ['workflow', 'set', 'sideways'],
        ['project', 'workflow', 'set', 'sideways'],
      ],
    ]);
  });

  test('logo and the dashboard’s project list read and change the same', () => {
    parity(
      [
        [['logo'], ['project', 'logo']],
        [
          ['logo', 'set', 'logo.svg'],
          ['project', 'logo', 'set', 'logo.svg'],
        ],
        [
          ['logo', '--json'],
          ['-j', 'project', 'logo'],
        ],
        [
          ['logo', 'unset'],
          ['project', 'logo', 'unset'],
        ],
        [
          ['logo', 'unset'],
          ['project', 'logo', 'unset'],
        ],
        [
          ['projects', 'list'],
          ['project', 'list'],
        ],
        [
          ['projects', 'add'],
          ['project', 'add'],
        ],
        [
          ['projects', 'add'],
          ['project', 'add'],
        ],
        [
          ['projects', 'list', '--json'],
          ['-j', 'project', 'list'],
        ],
        [
          ['projects', 'prune'],
          ['project', 'prune'],
        ],
        [
          ['projects', 'remove', 'demo'],
          ['project', 'remove', 'demo'],
        ],
        [
          ['projects', 'remove', 'demo'],
          ['project', 'remove', 'demo'],
        ],
      ],
      WITH_ID,
    );
  });
});

describe('atm2 sprint group', { skip: !available && 'atm2 needs Typer (uv sync --dev)' }, () => {
  test('a sprint from add to close, and the backlog, the same', () => {
    parity([
      [['sprints'], ['sprint', 'list']],
      [
        ['sprints', 'add', 'First', '--goal', 'The basics', '--release', '1.0'],
        ['sprint', 'add', 'First', '--goal', 'The basics', '--release', '1.0'],
      ],
      [
        ['sprints', 'add', 'Second', '--after', 'First', '--json'],
        ['-j', 'sprint', 'add', 'Second', '--after', 'First'],
      ],
      [
        ['sprints', 'edit', 'First', '--due', '2026-12-01'],
        ['sprint', 'edit', 'First', '--due', '2026-12-01'],
      ],
      [
        ['move', 'T-1', 'T-3', '--to', 'First'],
        ['sprint', 'move', 'T-1', 'T-3', '--to', 'First'],
      ],
      [
        ['move', 'T-3', '--to', 'First', '--json'],
        ['-j', 'sprint', 'move', 'T-3', '--to', 'First'],
      ],
      [['backlog'], ['sprint', 'backlog']],
      [
        ['backlog', '--json', '--all'],
        ['-j', 'sprint', 'backlog', '--all'],
      ],
      [
        ['sprints', 'start', 'First'],
        ['sprint', 'start', 'First'],
      ],
      [['sprints'], ['sprint', 'list']],
      [
        ['status', '--sprint', 'First'],
        ['project', 'status', '--sprint', 'First'],
      ],
      [
        ['sprints', 'start', 'Nope'],
        ['sprint', 'start', 'Nope'],
      ],
      [
        ['start', 'T-1'],
        ['task', 'state', 'T-1', 'in_progress'],
      ],
      [
        ['done', 'T-1'],
        ['task', 'state', 'T-1', 'done'],
      ],
      [
        ['sprints', 'close', 'First', '--carry-to', 'backlog'],
        ['sprint', 'close', 'First', '--carry-to', 'backlog'],
      ],
      [
        ['sprints', 'list', '--all', '--json'],
        ['-j', 'sprint', 'list', '--all'],
      ],
      [
        ['sprints', 'add', 'Mistake'],
        ['sprint', 'add', 'Mistake'],
      ],
      [
        ['sprints', 'delete', 'Mistake'],
        ['sprint', 'delete', 'Mistake'],
      ],
      [
        ['sprints', 'delete', 'First'],
        ['sprint', 'delete', 'First'],
      ],
    ]);
  });
});

describe('atm2 infra commands, kept apart', { skip: !available && 'atm2 needs Typer (uv sync --dev)' }, () => {
  const WIDE = { ...ENV, COLUMNS: '120' };

  test('--help lists the everyday commands first; Maintenance and Meta after them', () => {
    const root = makeProject(SETUP);
    try {
      const help = runAtm2(['--help'], root, WIDE);
      assert.equal(help.code, 0, help.stderr);
      const at = (word) => {
        const i = help.stdout.search(new RegExp(`\\b${word}\\b`));
        assert.notEqual(i, -1, `${word} in atm2 --help:\n${help.stdout}`);
        return i;
      };
      const everyday = ['task', 'loop', 'claim', 'vcs', 'release', 'component', 'project', 'sprint'];
      const maintenance = at('Maintenance');
      for (const name of everyday) assert.ok(at(name) < maintenance, `${name} before Maintenance`);
      for (const name of ['db', 'doctor', 'service']) assert.ok(at(name) > maintenance, `${name} under Maintenance`);
      const meta = at('Meta');
      for (const name of ['help', 'version'])
        assert.ok(help.stdout.indexOf(` ${name} `, meta) > meta, `${name} under Meta`);
    } finally {
      removeDir(root);
    }
  });

  test('db merge-driver and project summary work but are hidden', () => {
    const root = makeProject(SETUP);
    try {
      const db = runAtm2(['db', '--help'], root, WIDE);
      assert.equal(db.code, 0, db.stderr);
      assert.doesNotMatch(db.stdout, /merge-driver/);
      assert.match(db.stdout, /init/);
      const project = runAtm2(['project', '--help'], root, WIDE);
      assert.doesNotMatch(project.stdout, /summary/);
      assert.match(project.stdout, /config/);
    } finally {
      removeDir(root);
    }
  });

  test('help prints the same list as --help', () => {
    const root = makeProject(SETUP);
    try {
      const help = runAtm2(['help'], root, WIDE);
      assert.equal(help.code, 0, help.stderr);
      assert.equal(help.stdout, runAtm2(['--help'], root, WIDE).stdout);
    } finally {
      removeDir(root);
    }
  });

  test('version, summary and the merge driver give what atm gives', () => {
    parity([
      [['version'], ['version']],
      [
        ['version', '--json'],
        ['-j', 'version'],
      ],
      [['summary'], ['project', 'summary']],
    ]);
  });

  test('the merge driver merges both branches’ tasks, and reports a conflict, as atm does', () => {
    const sides = (dir, ours) => {
      const write = (name, tasks) => {
        const file = path.join(dir, name);
        fs.writeFileSync(file, JSON.stringify({ tasks }, null, 2) + '\n');
        return file;
      };
      const base = [{ id: 'T-1', name: 'One', type: 'feature', status: 'todo' }];
      return [
        write('base.json', base),
        write('ours.json', [{ ...base[0], name: ours }]),
        write('theirs.json', [...base, { id: 'T-2', name: 'Two', type: 'feature', status: 'todo' }]),
      ];
    };
    for (const [oursName, theirsName] of [
      ['One, renamed', null],
      ['Ours', 'Theirs'],
    ]) {
      const one = makeProject({});
      const two = makeProject({});
      try {
        const [b1, o1, t1] = sides(one, oursName);
        const [b2, o2, t2] = sides(two, oursName);
        if (theirsName) {
          // Both branches renamed T-1: a conflict
          for (const t of [t1, t2]) {
            const doc = JSON.parse(fs.readFileSync(t, 'utf8'));
            doc.tasks[0].name = theirsName;
            fs.writeFileSync(t, JSON.stringify(doc, null, 2) + '\n');
          }
        }
        const expected = runAtm(['merge-driver', b1, o1, t1], one, ENV);
        const actual = runAtm2(['db', 'merge-driver', b2, o2, t2], two);
        assert.deepEqual(relative(actual, two, two), relative(expected, one, one));
        assert.equal(fs.readFileSync(o2, 'utf8'), fs.readFileSync(o1, 'utf8'));
      } finally {
        removeDir(one);
        removeDir(two);
      }
    }
  });
});

/** What the shell would offer after `line` (fish's format: one `value<TAB>help` per line). */
function complete(line, cwd) {
  const env = {
    ...ENV,
    _ATM2_COMPLETE: 'complete_fish',
    _TYPER_COMPLETE_FISH_ACTION: 'get-args',
    _TYPER_COMPLETE_ARGS: line,
  };
  const result = runAtm2([], cwd, env);
  assert.equal(result.code, 0, result.stderr);
  return result.stdout.split('\n').filter(Boolean);
}

describe('atm2 shell completion', { skip: !available && 'atm2 needs Typer (uv sync --dev)' }, () => {
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
    const root = makeProject(SPRINTS);
    try {
      assert.deepEqual(complete('atm2 task details T-', root), ['T-1\tFirst', 'T-2\tSecond', 'T-3\tThird']);
      assert.deepEqual(complete('atm2 task deps T-2', root), ['T-2\tSecond']);
      assert.deepEqual(complete('atm2 task list --sprint ', root), ['First\tThe basics']);
      assert.deepEqual(complete('atm2 task list --status a', root), ['awaiting_user\tAwaiting you']);
      assert.deepEqual(complete('atm2 task list --priority P', root), [
        'P0\tCritical',
        'P1\tHigh',
        'P2\tMedium',
        'P3\tLow',
      ]);
      assert.deepEqual(complete('atm2 task state T-1 in_', root), ['in_progress\tstart', 'in_review\treview']);
      assert.deepEqual(complete('atm2 sprint start ', root), ['First\tThe basics']);
    } finally {
      removeDir(root);
    }
  });

  test('outside a project, or with a task file atm can’t read, it completes nothing', () => {
    const empty = makeProject({});
    const broken = makeProject({ files: { '.agentic-task-manager/tasks.json': '{ not json' } });
    try {
      assert.deepEqual(complete('atm2 task details T-', empty), []);
      assert.deepEqual(complete('atm2 task list --sprint ', broken), []);
    } finally {
      removeDir(empty);
      removeDir(broken);
    }
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
