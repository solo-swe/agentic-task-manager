// atm commit, atm pr and atm merged (docs/v4/atm-cli.md §4.5, data-model.md §4): commit stages exactly the files
// named plus tasks.json, builds the message from meta.workflow, and records the commit on the task.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { describe, test } from 'node:test';
import { git, makeProject, removeDir, runAtm } from './helpers.mjs';

const ENV = { ATM_AGENT: 'alice', ATM_NOW: '2026-05-01T12:00:00Z' };
const TASKS = '.agentic-task-manager/tasks.json';

function project(meta = {}, tasks = [{ id: 'T-001', name: 'Add login', phase: 1, type: 'feature', status: 'done' }]) {
  return makeProject({ commit: true, files: { [TASKS]: { meta, tasks }, 'src/a.txt': 'a\n', 'src/b.txt': 'b\n' } });
}

const read = (root) => JSON.parse(fs.readFileSync(path.join(root, TASKS), 'utf8'));
const write = (root, file, text) => fs.writeFileSync(path.join(root, file), text);
const committedFiles = (root) => git(['show', '--name-only', '--format=', 'HEAD'], root).split('\n').sort();

describe('atm commit', () => {
  test('commits only the named files and tasks.json; other changes, staged or not, stay out', () => {
    const root = project();
    try {
      write(root, 'src/a.txt', 'a2\n');
      write(root, 'src/b.txt', 'b2\n');
      write(root, 'src/new.txt', 'new\n');
      write(root, 'other.txt', 'someone else\n');
      git(['add', 'other.txt'], root); // another agent's staged work
      const tasks = read(root);
      tasks.tasks[0].notes = [{ text: 'changed by atm' }];
      write(root, TASKS, JSON.stringify(tasks, null, 2) + '\n');

      const result = runAtm(['vcs', 'commit', 'T-001', '--file', 'src/a.txt', '--file', 'src/new.txt'], root, ENV);
      assert.equal(result.code, 0, result.stderr);
      const sha = git(['rev-parse', 'HEAD'], root);
      assert.match(result.stdout, new RegExp(`^T-001: committed ${sha.slice(0, 7)} feat\\(T-001\\): Add login\\n`));
      assert.deepEqual(committedFiles(root), [TASKS, 'src/a.txt', 'src/new.txt']);
      assert.equal(git(['log', '-1', '--format=%B'], root), 'feat(T-001): Add login');
      // other.txt is still staged, b.txt still modified
      assert.match(git(['status', '--porcelain'], root), /^A {2}other\.txt$/m);
      assert.match(git(['status', '--porcelain'], root), /^ M src\/b\.txt$/m);
      // The SHA is recorded afterwards, as a change for the next commit
      const task = read(root).tasks[0];
      assert.equal(task.commit, sha);
      assert.equal(task.updated_by, 'alice');
    } finally {
      removeDir(root);
    }
  });

  test('a deleted file can be named; paths are relative to where atm runs', () => {
    const root = project();
    try {
      fs.rmSync(path.join(root, 'src/b.txt'));
      const result = runAtm(['vcs', 'commit', 'T-001', '--file', 'b.txt'], path.join(root, 'src'), ENV);
      assert.equal(result.code, 0, result.stderr);
      assert.deepEqual(committedFiles(root), ['src/b.txt']);
      assert.equal(git(['ls-files', 'src/b.txt'], root), '');
    } finally {
      removeDir(root);
    }
  });

  test('a file already removed with git rm can be named too, alongside changed files', () => {
    const root = project();
    try {
      git(['rm', '-q', 'src/b.txt'], root);
      write(root, 'src/a.txt', 'a2\n');
      const result = runAtm(['vcs', 'commit', 'T-001', '--file', 'src/a.txt', '--file', 'src/b.txt'], root, ENV);
      assert.equal(result.code, 0, result.stderr);
      assert.deepEqual(committedFiles(root), ['src/a.txt', 'src/b.txt']);
      // Only the recorded SHA is left over (git() trims the status line's leading space)
      assert.equal(git(['status', '--porcelain'], root), 'M .agentic-task-manager/tasks.json');
    } finally {
      removeDir(root);
    }
  });

  test('refuses whole-tree pathspecs, directories, paths outside the repository, and a missing --file', () => {
    const root = project();
    try {
      for (const [file, message] of [
        ['.', /would stage the whole tree/],
        ['-A', /would stage the whole tree/],
        [':/', /would stage the whole tree/],
        ['src', /is a directory/],
        ['../elsewhere.txt', /outside the repository/],
      ]) {
        const result = runAtm(['vcs', 'commit', 'T-001', `--file=${file}`], root, ENV);
        assert.equal(result.code, 2, `${file}: ${result.stderr}`);
        assert.match(result.stderr, message);
      }
      const missing = runAtm(['vcs', 'commit', 'T-001'], root, ENV);
      assert.equal(missing.code, 2);
      assert.match(missing.stderr, /--file/);
      const gone = runAtm(['vcs', 'commit', 'T-001', '--file', 'nope.txt'], root, ENV);
      assert.equal(gone.code, 1);
      assert.match(gone.stderr, /no such file/);
      assert.equal(git(['rev-list', '--count', 'HEAD'], root), '1');
    } finally {
      removeDir(root);
    }
  });

  test('a --file is one file, never a pattern: *.env stages no untracked secrets; [id] is a real name (T-223)', () => {
    const root = makeProject({
      commit: true,
      files: {
        [TASKS]: { tasks: [{ id: 'T-001', name: 'Add login', phase: 1, type: 'feature', status: 'done' }] },
        'a.env': 'TRACKED=1\n',
        'src/[id]/page.ts': 'export {};\n',
        'src/i': 'a file the pattern [id] would match\n',
      },
    });
    try {
      write(root, 'a.env', 'TRACKED=2\n');
      write(root, 'secret.env', 'TOKEN=hunter2\n');
      const pattern = runAtm(['vcs', 'commit', 'T-001', '--file', '*.env'], root, ENV);
      assert.equal(pattern.code, 1, pattern.stderr);
      assert.match(pattern.stderr, /no such file/);
      assert.equal(git(['rev-list', '--count', 'HEAD'], root), '1');
      assert.match(git(['status', '--porcelain'], root), /^\?\? secret\.env$/m); // still untracked

      write(root, 'src/[id]/page.ts', 'export const page = 1;\n');
      write(root, 'src/i', 'changed, and not named\n');
      const literal = runAtm(['vcs', 'commit', 'T-001', '--file', 'src/[id]/page.ts'], root, ENV);
      assert.equal(literal.code, 0, literal.stderr);
      assert.deepEqual(committedFiles(root), ['src/[id]/page.ts']); // not src/i; tasks.json had no change
    } finally {
      removeDir(root);
    }
  });

  test('the template and the trailer rule shape the message; --message replaces the template', () => {
    const forbid = project({
      workflow: { type: 'direct', commit_template: '{type}: {name} ({id})', trailers: 'forbid' },
    });
    const require = project({
      workflow: { type: 'direct', trailers: { require: ['Signed-off-by: Alice <a@example.com>'] } },
    });
    try {
      write(forbid, 'src/a.txt', 'a2\n');
      const msg = 'fix: {name}\n\nCo-Authored-By: Claude <noreply@anthropic.com>';
      assert.equal(runAtm(['vcs', 'commit', 'T-001', '--file', 'src/a.txt', '--message', msg], forbid, ENV).code, 0);
      assert.equal(git(['log', '-1', '--format=%B'], forbid), 'fix: Add login');
      write(forbid, 'src/a.txt', 'a3\n');
      assert.equal(runAtm(['vcs', 'commit', 'T-001', '--file', 'src/a.txt'], forbid, ENV).code, 0);
      assert.equal(git(['log', '-1', '--format=%B'], forbid), 'feature: Add login (T-001)');

      write(require, 'src/a.txt', 'a2\n');
      const result = runAtm(['vcs', 'commit', 'T-001', '--file', 'src/a.txt', '--json'], require, ENV);
      assert.equal(result.code, 0, result.stderr);
      const expected = 'feat(T-001): Add login\n\nSigned-off-by: Alice <a@example.com>';
      assert.equal(git(['log', '-1', '--format=%B'], require), expected);
      const json = JSON.parse(result.stdout);
      assert.deepEqual(json, {
        task: 'T-001',
        commit: git(['rev-parse', 'HEAD'], require),
        message: expected,
        files: ['src/a.txt', TASKS],
      });
    } finally {
      removeDir(forbid);
      removeDir(require);
    }
  });

  test('nothing to commit is an error from git, and nothing is recorded', () => {
    const root = project();
    try {
      const result = runAtm(['vcs', 'commit', 'T-001', '--file', 'src/a.txt'], root, ENV);
      assert.equal(result.code, 1);
      assert.match(result.stderr, /git commit failed/);
      assert.equal(read(root).tasks[0].commit, undefined);
    } finally {
      removeDir(root);
    }
  });
});

// T-343: after a rebase or cherry-pick, the recorded SHA is no longer on the branch; --record names the new one
describe('atm commit --dir (T-405)', () => {
  /** A project with a generated folder gen/ (two files committed), and src/ */
  function generated(tasks) {
    const root = makeProject({
      commit: true,
      files: {
        [TASKS]: {
          meta: {},
          tasks: tasks ?? [{ id: 'T-001', name: 'Rebuild', phase: 1, type: 'infra', status: 'in_progress' }],
        },
        'src/a.txt': 'a\n',
        'gen/old.js': 'old\n',
        'gen/keep.js': 'keep\n',
      },
    });
    return root;
  }

  test('commits every change under the folder (modified, new, deleted, nested) and nothing outside it', () => {
    const root = generated();
    try {
      write(root, 'gen/keep.js', 'keep2\n');
      fs.rmSync(path.join(root, 'gen/old.js'));
      fs.mkdirSync(path.join(root, 'gen/chunks'));
      write(root, 'gen/chunks/new.js', 'new\n');
      write(root, 'src/a.txt', 'a2\n');
      const result = runAtm(['vcs', 'commit', 'T-001', '--dir', 'gen'], root, ENV);
      assert.equal(result.code, 0, result.stderr);
      assert.deepEqual(committedFiles(root), ['gen/chunks/new.js', 'gen/keep.js', 'gen/old.js']);
      assert.match(git(['status', '--porcelain'], root), /^ ?M src\/a\.txt$/m);
      // With --file beside it, and from a subfolder
      write(root, 'gen/keep.js', 'keep3\n');
      write(root, 'src/a.txt', 'a3\n');
      const both = runAtm(
        ['vcs', 'commit', 'T-001', '--dir', '../gen', '--file', 'a.txt'],
        path.join(root, 'src'),
        ENV,
      );
      assert.equal(both.code, 0, both.stderr);
      assert.deepEqual(committedFiles(root), [TASKS, 'gen/keep.js', 'src/a.txt']);
    } finally {
      removeDir(root);
    }
  });

  test('refuses the root, above it, the task folder, a file, and a folder with nothing changed', () => {
    const root = generated();
    try {
      write(root, 'gen/keep.js', 'keep2\n');
      for (const [dir, code, message] of [
        ['.', 2, /not the repository itself/],
        ['..', 2, /not the repository itself/],
        ['.agentic-task-manager', 2, /task folder/],
        ['src/a.txt', 2, /is a file/],
        ['src', 1, /nothing changed under it/],
      ]) {
        const result = runAtm(['vcs', 'commit', 'T-001', '--dir', dir], root, ENV);
        assert.equal(result.code, code, `${dir}: ${result.stderr}`);
        assert.match(result.stderr, message, dir);
      }
      assert.equal(git(['rev-list', '--count', 'HEAD'], root), '1');
    } finally {
      removeDir(root);
    }
  });

  test('refuses, committing nothing, when a changed file under it is in another agent’s claimed task', () => {
    const root = generated([
      { id: 'T-001', name: 'Rebuild', phase: 1, type: 'infra', status: 'todo' },
      { id: 'T-002', name: 'Hand-edit', phase: 1, type: 'feature', status: 'todo', files: ['gen/keep.js'] },
    ]);
    try {
      assert.equal(runAtm(['task', 'state', 'T-001', 'in_progress'], root, ENV).code, 0);
      assert.equal(runAtm(['task', 'state', 'T-002', 'in_progress'], root, { ...ENV, ATM_AGENT: 'bob' }).code, 0);
      write(root, 'gen/keep.js', 'bob was here\n');
      write(root, 'gen/new.js', 'new\n');
      const result = runAtm(['vcs', 'commit', 'T-001', '--dir', 'gen'], root, ENV);
      assert.equal(result.code, 1, result.stderr);
      assert.match(result.stderr, /gen\/keep\.js.*T-002.*bob/);
      assert.equal(git(['rev-list', '--count', 'HEAD'], root), '1');
      // Naming its own files still works
      assert.equal(runAtm(['vcs', 'commit', 'T-001', '--file', 'gen/new.js'], root, ENV).code, 0);
    } finally {
      removeDir(root);
    }
  });
});

describe('atm commit --record', () => {
  /** A project with a commit for T-001 on top of the initial one; returns [root, its SHA] */
  function withTaskCommit(message = 'feat(T-001): Add login') {
    const root = project();
    write(root, 'src/a.txt', 'a2\n');
    git(['commit', '-q', '-am', message], root);
    return [root, git(['rev-parse', 'HEAD'], root)];
  }

  test('records an existing commit, by SHA or by ref, as its full SHA; nothing is committed', () => {
    const [root, sha] = withTaskCommit();
    try {
      const count = git(['rev-list', '--count', 'HEAD'], root);
      const short = runAtm(['vcs', 'commit', 'T-001', '--record', sha.slice(0, 8)], root, ENV);
      assert.equal(short.code, 0, short.stderr);
      assert.equal(short.stdout, `T-001: recorded ${sha.slice(0, 7)} feat(T-001): Add login\n`);
      assert.equal(read(root).tasks[0].commit, sha);
      assert.equal(read(root).tasks[0].updated_by, 'alice');
      assert.equal(git(['rev-list', '--count', 'HEAD'], root), count);

      const json = runAtm(['vcs', 'commit', 'T-001', '--record', 'HEAD', '--json'], root, ENV);
      assert.equal(json.code, 0, json.stderr);
      assert.deepEqual(JSON.parse(json.stdout), { task: 'T-001', commit: sha, recorded: true });
    } finally {
      removeDir(root);
    }
  });

  test('a commit whose message does not name the task is recorded, with a hint', () => {
    const [root, sha] = withTaskCommit('Tidy up');
    try {
      const result = runAtm(['vcs', 'commit', 'T-001', '--record', sha], root, ENV);
      assert.equal(result.code, 0, result.stderr);
      assert.match(result.stdout, /its message doesn.t mention T-001/);
      assert.equal(read(root).tasks[0].commit, sha);
      const json = JSON.parse(runAtm(['vcs', 'commit', 'T-001', '--record', sha, '--json'], root, ENV).stdout);
      assert.match(json.hint, /doesn.t mention T-001/);
    } finally {
      removeDir(root);
    }
  });

  test('refuses an unknown revision, an option, a commit not on the branch, and --record with --file', () => {
    const [root] = withTaskCommit();
    try {
      // A commit on another branch only
      git(['switch', '-q', '-c', 'side'], root);
      write(root, 'src/b.txt', 'side\n');
      git(['commit', '-q', '-am', 'feat(T-001): on the side'], root);
      const side = git(['rev-parse', 'HEAD'], root);
      git(['switch', '-q', '-'], root);
      const tree = git(['rev-parse', 'HEAD^{tree}'], root);
      for (const [args, code, message] of [
        [['--record', 'nope'], 1, /no commit nope/],
        [['--record=--output=x'], 2, /a revision, not an option/],
        [['--record', side], 1, /not on the current branch/],
        [['--record', tree], 1, /no commit/],
        [
          ['--record', 'HEAD', '--file', 'src/a.txt'],
          2,
          /--record records an existing commit: no --file, --dir or --message/,
        ],
        [['--record', 'HEAD', '--dir', 'src'], 2, /--record records an existing commit/],
      ]) {
        const result = runAtm(['vcs', 'commit', 'T-001', ...args], root, ENV);
        assert.equal(result.code, code, `${args.join(' ')}: ${result.stderr}`);
        assert.match(result.stderr, message);
      }
      assert.equal(read(root).tasks[0].commit, undefined);
      assert.ok(!fs.existsSync(path.join(root, 'x')));
    } finally {
      removeDir(root);
    }
  });
});
