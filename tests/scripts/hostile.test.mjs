// Task files atm didn't write: deeply nested JSON, tasks that aren't objects, a dependency chain thousands long, an ID
// built to make a regex backtrack, a server record that came with the repository. atm answers each with a clean error
// or does its job, never a traceback, a hang or a signal to someone else's process (docs/security-review.md F12-F14,
// T-228).
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { describe, test } from 'node:test';
import { git, makeProject, removeDir, runAtm, tempDir } from './helpers.mjs';

const TASKS = '.agentic-task-manager/tasks.json';
const task = (id, extra = {}) => ({ id, name: id, phase: 0, type: 'feature', status: 'todo', ...extra });
const env = (root) => ({ ATM_HOME: path.join(root, '.atm-home') });

describe('hostile task files', () => {
  test('deeply nested JSON: a clean "not a task file" error, no traceback', () => {
    const root = makeProject({ files: { 'README.md': 'x\n' } });
    try {
      fs.mkdirSync(path.join(root, '.agentic-task-manager'));
      fs.writeFileSync(path.join(root, TASKS), '['.repeat(100000) + ']'.repeat(100000));
      const r = runAtm(['project', 'status'], root, env(root));
      assert.equal(r.code, 1, r.stderr);
      assert.match(r.stderr, /not a valid task file/);
      assert.doesNotMatch(r.stderr, /Traceback/);
    } finally {
      removeDir(root);
    }
  });

  test('a task that is not an object, or dependencies that are not IDs: a clean error naming it', () => {
    for (const tasks of [
      [task('T-1'), 'T-2'],
      [task('T-1', { dependencies: 'T-2' })],
      [task('T-1', { dependencies: [7] })],
    ]) {
      const root = makeProject({ files: { [TASKS]: { tasks } } });
      try {
        const r = runAtm(['project', 'status'], root, env(root));
        assert.equal(r.code, 1, JSON.stringify(tasks));
        assert.match(r.stderr, /run atm doctor/);
        assert.doesNotMatch(r.stderr, /Traceback/);
      } finally {
        removeDir(root);
      }
    }
  });

  test('a 3000-task dependency chain: checked without hitting the recursion limit; a cycle through it is named', () => {
    const chain = Array.from({ length: 3000 }, (_, i) => task(`T-${i + 1}`, i ? { dependencies: [`T-${i}`] } : {}));
    const root = makeProject({ files: { [TASKS]: { tasks: chain } } });
    try {
      const ok = runAtm(['doctor', '--json'], root, env(root));
      assert.equal(ok.code, 0, ok.stderr);
      chain[0].dependencies = ['T-3000'];
      fs.writeFileSync(path.join(root, TASKS), JSON.stringify({ tasks: chain }));
      const cycle = runAtm(['doctor', '--json'], root, env(root));
      assert.equal(cycle.code, 1, cycle.stderr);
      assert.match(JSON.parse(cycle.stdout).errors.join('\n'), /dependency cycle: T-1 -> T-3000 -> .* -> T-1/);
    } finally {
      removeDir(root);
    }
  });

  test('an ID of 20000 digits and a letter: the merge driver renames it at once (no regex backtracking)', () => {
    const dir = tempDir();
    try {
      const id = '1'.repeat(20000) + 'x';
      const files = ['base', 'ours', 'theirs'].map((n) => path.join(dir, n));
      fs.writeFileSync(files[0], JSON.stringify({ tasks: [] }));
      fs.writeFileSync(files[1], JSON.stringify({ tasks: [{ id, name: 'ours', status: 'todo' }] }));
      fs.writeFileSync(files[2], JSON.stringify({ tasks: [{ id, name: 'theirs', status: 'todo' }] }));
      const started = Date.now();
      const r = runAtm(['db', 'merge-driver', ...files], dir);
      assert.equal(r.code, 0, r.stderr);
      assert.ok(Date.now() - started < 5000, `took ${Date.now() - started} ms`);
      const ids = JSON.parse(fs.readFileSync(files[1], 'utf8')).tasks.map((t) => t.id);
      assert.deepEqual(ids, [id, `${id}-2`]);
    } finally {
      removeDir(dir);
    }
  });

  test('a server record that came with the repository is deleted; its port is not asked, its pid not signalled', async () => {
    const root = makeProject({ git: true, commit: true, files: { [TASKS]: { tasks: [] }, 'README.md': 'x\n' } });
    // A process the record names, and a listener on its port that answers as that project's old server would
    const victim = spawn(process.execPath, ['-e', 'setTimeout(() => {}, 60000)'], { stdio: 'ignore' });
    let asked = false;
    const server = http.createServer((req, res) => {
      asked = true;
      res.end(JSON.stringify({ atm: true, folder: path.join(root, '.agentic-task-manager'), pid: victim.pid }));
    });
    await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
    try {
      const record = path.join(root, '.agentic-task-manager/.server.json');
      fs.writeFileSync(record, JSON.stringify({ pid: victim.pid, port: server.address().port }));
      git(['add', '-f', '.agentic-task-manager/.server.json'], root);
      git(['commit', '-q', '-m', 'a record from someone else'], root);
      const r = runAtm(['db', 'migrate'], root, env(root));
      assert.equal(r.code, 0, r.stderr);
      assert.equal(fs.existsSync(record), false);
      assert.equal(asked, false, 'the port was asked');
      assert.equal(victim.exitCode, null, 'the process was signalled');
      assert.equal(victim.signalCode, null, 'the process was signalled');
    } finally {
      victim.kill();
      server.close();
      removeDir(root);
    }
  });
});
