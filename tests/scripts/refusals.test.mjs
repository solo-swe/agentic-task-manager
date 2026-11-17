// Options that can't go together, a required one missing, a value out of range, a file that isn't there: each refused
// with atm's exit code (2 a usage error, 1 another error: docs/agent/commands.md), naming the problem, and nothing
// written. Refusals of one command's rules are in its scenarios; these are the commands' own checks of their options.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { after, before, describe, test } from 'node:test';
import { makeProject, removeDir, runAtm } from './helpers.mjs';

// [args, exit code, what stderr says]
const CASES = [
  // loop next: one way of choosing the tasks at a time
  [
    ['loop', 'next', '--sprint', 'S1', '--all'],
    2,
    "atm: --sprint and --all can't both be given: they choose the tasks in different ways",
  ],
  [['loop', 'next', '--tasks', 'T-1', '--backlog'], 2, "atm: --tasks and --backlog can't both be given"],
  [['loop', 'next', '--all', '--backlog', '--claim'], 2, "atm: --all and --backlog can't both be given"],
  [
    ['loop', 'next', '--sprint', 'S1', '--tasks', 'T-1', '--all'],
    2,
    "atm: --sprint and --tasks and --all can't both be given",
  ],
  // task add, task edit
  [
    ['task', 'add', '--name', 'x', '--type', 'feature', '--description', 'd', '--description-file', 'd.md'],
    2,
    "atm: --description and --description-file can't both be given",
  ],
  [
    ['task', 'add', '--name', 'x', '--type', 'feature', '--description-file', 'gone.md'],
    1,
    "atm: can't read the description file gone.md",
  ],
  [
    ['task', 'edit', 'T-1', '--description', 'd', '--description-file', 'd.md'],
    2,
    "atm: --description and --description-file can't both be given",
  ],
  [['task', 'edit', 'T-1', '--sprint', 'S1', '--backlog'], 2, "atm: --sprint and --backlog can't both be given"],
  [['task', 'edit', 'T-1', '--description-file', 'gone.md'], 1, "atm: can't read the description file gone.md"],
  [
    ['task', 'add', '--name', 'x', '--type', 'feature', '--components', 'all'],
    2,
    "invalid value 'all' for '--components",
  ],
  // sprint, vcs: required options
  [['sprint', 'move', 'T-1'], 2, "atm: Missing option '--to'."],
  [['sprint', 'move', '--to', 'S1'], 2, 'required'],
  [['vcs', 'pushed'], 2, "atm: Missing option '--pr'."],
  [['vcs', 'pr', 'T-1', '--url', 'https://e.com/1'], 2, "atm: Missing option '--number'."],
  [['vcs', 'pr', '--number', '1', '--url', 'https://e.com/1'], 2, 'required'],
  [['vcs', 'pr', 'T-1', '--number', 'one', '--url', 'u'], 2, "invalid value 'one' for '--number"],
  [['vcs', 'repository', '--from', 'https://old.example'], 2, "unexpected argument 'https://old.example'"],
  // claim hold: its bounds
  [['claim', 'hold', 'T-2', '--every', '0'], 2, 'atm: --every must be more than 0 seconds'],
  [['claim', 'hold', 'T-2', '--every=-1'], 2, 'atm: --every must be more than 0 seconds'],
  // project
  [['project', 'logo', 'set'], 2, 'atm: atm project logo set needs a path'],
  // (the path without `set` reads as the action)
  [['project', 'logo', 'logo.svg'], 2, "atm: action: invalid choice 'logo.svg' (choose from set, unset)"],
  [['project', 'workflow', 'get'], 2, "atm: action: invalid choice 'get' (choose from set)"],
  [
    ['project', 'workflow', 'set', 'direct', '--verify', 'maybe'],
    2,
    "atm: --verify: invalid choice 'maybe' (choose from true, false)",
  ],
  [
    ['project', 'workflow', 'set', 'direct', '--trailers', 'some'],
    2,
    "atm: --trailers: invalid choice 'some' (choose from forbid, require, none)",
  ],
  [['project', 'vocab', 'rename', 'label', 'x', 'y'], 2, "invalid value 'label'"],
  // release edit: --force only with --released, --released and --planned not together
  [['release', 'edit', '1.0', '--released', '--planned'], 2, 'cannot be used with'],
  // service, completion, docs
  [['service', '--dir', 'nowhere'], 2, 'atm: --dir nowhere: not a directory'],
  [['service', 'restart'], 2, "invalid value 'restart'"],
  [['service', '--port', '70000'], 2, "invalid value '70000' for '--port"],
  [['completion', 'tcsh'], 2, "invalid value 'tcsh'"],
  [['completion'], 2, 'required'],
];

describe('refused options', () => {
  let root;
  let env;
  let before_;
  /** The project's files and text */
  const files = () =>
    Object.fromEntries(
      fs
        .readdirSync(root, { recursive: true })
        .filter((f) => !f.startsWith('.git') && fs.statSync(path.join(root, f)).isFile())
        .map((f) => [f, fs.readFileSync(path.join(root, f), 'utf8')]),
    );
  before(() => {
    root = makeProject({
      git: true,
      files: {
        '.agentic-task-manager/tasks.json': {
          meta: {
            project: 'Refusals',
            sprints: [{ id: 'S1', state: 'active' }],
            releases: [{ id: '1.0' }],
          },
          tasks: [
            {
              id: 'T-1',
              name: 'One',
              type: 'feature',
              status: 'todo',
              sprint: 'S1',
            },
            {
              id: 'T-2',
              name: 'Two',
              type: 'feature',
              status: 'in_progress',
              started_by: 'bob',
            },
          ],
        },
        'd.md': 'A description.\n',
        'logo.svg': '<svg xmlns="http://www.w3.org/2000/svg"/>',
      },
    });
    env = { ATM_HOME: path.join(root, '.atm-home'), ATM_AGENT: 'bob' };
    assert.equal(runAtm(['project', 'status'], root, env).code, 0);
    before_ = files();
  });
  after(() => removeDir(root));

  for (const [args, code, said] of CASES) {
    test(`atm ${args.join(' ')}: exit ${code}`, () => {
      const r = runAtm(args, root, env);
      assert.equal(r.code, code, `stdout:\n${r.stdout}\nstderr:\n${r.stderr}`);
      assert.ok(r.stderr.includes(said), `stderr should include "${said}":\n${r.stderr}`);
      assert.equal(r.stdout, '');
      assert.deepEqual(files(), before_);
    });
  }

  test('with --json, each is a JSON error with the same code', () => {
    for (const [args, code] of CASES) {
      const r = runAtm([...args, '--json'], root, env);
      assert.equal(r.code, code, args.join(' '));
      assert.equal(JSON.parse(r.stderr).code, code, args.join(' '));
    }
  });
});

describe('no project here', () => {
  // Every command that works on a project, in a folder with no task file: exit 1, saying so, and nothing created
  const COMMANDS = [
    ['task', 'list'],
    ['task', 'details', 'T-1'],
    ['task', 'deps', 'T-1'],
    ['task', 'add', '--name', 'x', '--type', 'feature'],
    ['task', 'note', 'T-1', 'x'],
    ['task', 'edit', 'T-1', '--name', 'x'],
    ['task', 'state', 'T-1', 'in_progress'],
    ['task', 'review', 'approve', 'T-1'],
    ['loop', 'next'],
    ['claim', 'list'],
    ['claim', 'heartbeat', 'T-1'],
    ['review', 'list'],
    ['review', 'show', 'CR-1'],
    ['vcs', 'pr', 'T-1', '--number', '1', '--url', 'https://e.com/1'],
    ['vcs', 'pushed', '--pr', '1'],
    ['release', 'list'],
    ['release', 'add', '1.0'],
    ['component', 'list'],
    ['label', 'rename', 'a', 'b'],
    ['project', 'status'],
    ['project', 'workflow'],
    ['project', 'logo'],
    ['project', 'key', 'show'],
    ['project', 'vocab', 'rename', 'type', 'feature', 'feat'],
    ['sprint', 'list'],
    ['sprint', 'add', 'S1'],
    ['sprint', 'backlog'],
    ['db', 'unarchive', 'T-1'],
  ];
  // These leave an empty .agentic-task-manager/ folder behind before they refuse (no file in it): a bug in atm, kept
  // visible as a todo until it is fixed
  const LEAVE_EMPTY_FOLDER = new Set([
    'task add --name x --type feature',
    'loop next',
    'claim list',
    'vcs pushed --pr 1',
    'release add 1.0',
    'label rename a b',
    'project vocab rename type feature feat',
    'sprint add S1',
  ]);
  // These don't refuse yet (bugs in atm, todos until fixed): review list says "No reviews." and exits 0, review show
  // says "no review CR-1"
  const NO_REFUSAL = new Map([
    ['review list', 'lists no reviews instead of refusing'],
    ['review show CR-1', 'says there is no such review instead of no project'],
  ]);
  const files = (dir) =>
    fs.readdirSync(dir, { recursive: true }).filter((p) => !fs.statSync(path.join(dir, p)).isDirectory());
  for (const args of COMMANDS) {
    const name = args.join(' ');
    let created = null;
    test(`atm ${name}`, { todo: NO_REFUSAL.get(name) ?? false }, () => {
      const root = makeProject({});
      const home = makeProject({});
      try {
        const r = runAtm(args, root, { ATM_HOME: home });
        created = fs.readdirSync(root, { recursive: true });
        assert.equal(r.code, 1, `stdout:\n${r.stdout}\nstderr:\n${r.stderr}`);
        assert.match(r.stderr, /^atm: no tasks\.json in /);
        assert.deepEqual(files(root), []);
      } finally {
        removeDir(root);
        removeDir(home);
      }
    });
    test(
      `atm ${name}: not even a folder`,
      {
        todo: LEAVE_EMPTY_FOLDER.has(name) && 'leaves an empty .agentic-task-manager/',
      },
      () => {
        assert.deepEqual(created, []);
      },
    );
  }
});
