// The project registry ($ATM_HOME/projects.json): what scenarios can't express, git worktrees and several processes
// registering at once.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { describe, test } from 'node:test';
import { ATM, baseEnv, git, makeProject, removeDir, runAtm, tempDir } from './helpers.mjs';

const tasks = (id, project) => ({ meta: { id, project }, tasks: [] });
const registry = (home) => JSON.parse(fs.readFileSync(path.join(home, 'projects.json'), 'utf8'));

/** Start `atm args...` without waiting; resolves with its exit code and stderr. */
function startAtm(args, cwd, env) {
  const child = spawn(ATM, args, { cwd, env: { ...baseEnv(), ...env } });
  let stderr = '';
  child.stderr.on('data', (d) => (stderr += d));
  // `close`, not `exit`: all of stderr is read by then
  return new Promise((resolve) => child.on('close', (code) => resolve({ code, stderr })));
}

describe('project registry', () => {
  test('a linked git worktree registers its main worktree', () => {
    const root = makeProject({
      files: { '.agentic-task-manager/tasks.json': tasks('abcd1234', 'Demo') },
      commit: true,
    });
    const home = tempDir('atm-home-');
    const linked = path.join(tempDir('atm-wt-'), 'second');
    try {
      git(['worktree', 'add', '-q', '-b', 'second', linked], root);
      const r = runAtm(['project', 'add', '--json'], linked, { ATM_HOME: home });
      assert.equal(r.code, 0, r.stderr);
      assert.deepEqual(
        registry(home).projects.map((p) => [p.id, p.path]),
        [['abcd1234', root]],
      );
      // Adding the main worktree too doesn't make a second entry
      const again = runAtm(['project', 'add'], root, { ATM_HOME: home });
      assert.match(again.stdout, /^Already registered: abcd1234/);
      assert.equal(registry(home).projects.length, 1);
    } finally {
      git(['worktree', 'remove', '--force', linked], root);
      removeDir(path.dirname(linked));
      removeDir(home);
      removeDir(root);
    }
  });

  test('the hook (heartbeat --auto) lists a project once, then only reads the registry until a day has passed', () => {
    const root = makeProject({ files: { '.agentic-task-manager/tasks.json': tasks('abcd1234', 'Demo') } });
    const home = tempDir('atm-home-');
    const file = path.join(home, 'projects.json');
    const hook = (now) => runAtm(['claim', 'heartbeat', '--auto'], root, { ATM_HOME: home, ATM_NOW: now });
    try {
      assert.equal(hook('2026-01-01T00:00:00Z').code, 0);
      assert.deepEqual(registry(home).projects, [
        { id: 'abcd1234', name: 'Demo', path: root, added_at: '2026-01-01T00:00:00Z', seen_at: '2026-01-01T00:00:00Z' },
      ]);
      const written = fs.statSync(file).mtimeMs;
      // The next calls the same day don't write, even when the project's name changed
      fs.writeFileSync(path.join(root, '.agentic-task-manager/tasks.json'), JSON.stringify(tasks('abcd1234', 'New')));
      assert.equal(hook('2026-01-01T20:00:00Z').code, 0);
      assert.equal(fs.statSync(file).mtimeMs, written);
      // A day later it refreshes seen_at and the name
      assert.equal(hook('2026-01-02T01:00:00Z').code, 0);
      assert.deepEqual(
        registry(home).projects.map((p) => [p.name, p.seen_at]),
        [['New', '2026-01-02T01:00:00Z']],
      );
    } finally {
      removeDir(home);
      removeDir(root);
    }
  });

  test('the hook never waits for a busy registry lock, and never fails', () => {
    const root = makeProject({ files: { '.agentic-task-manager/tasks.json': tasks('abcd1234', 'Demo') } });
    const home = tempDir('atm-home-');
    try {
      // A live lock: held by this test process, on this host
      fs.mkdirSync(path.join(home, '.lock'));
      fs.writeFileSync(
        path.join(home, '.lock/owner.json'),
        JSON.stringify({ host: os.hostname(), pid: process.pid, acquired_at: new Date().toISOString() }),
      );
      const started = Date.now();
      const r = runAtm(['claim', 'heartbeat', '--auto'], root, { ATM_HOME: home });
      assert.equal(r.code, 0, r.stderr);
      assert.equal(r.stdout + r.stderr, '');
      assert.ok(Date.now() - started < 3000, 'did not wait for the lock');
      assert.equal(fs.existsSync(path.join(home, 'projects.json')), false);
    } finally {
      removeDir(home);
      removeDir(root);
    }
  });

  test('projects registering at the same time all end up in one valid file', async () => {
    const home = tempDir('atm-home-');
    const roots = Array.from({ length: 8 }, (_, i) =>
      makeProject({ files: { '.agentic-task-manager/tasks.json': tasks(`0000000${i}`, `P${i}`) } }),
    );
    try {
      const results = await Promise.all(roots.map((root) => startAtm(['project', 'add'], root, { ATM_HOME: home })));
      for (const r of results) assert.equal(r.code, 0, r.stderr);
      const ids = registry(home)
        .projects.map((p) => p.id)
        .sort();
      assert.deepEqual(
        ids,
        roots.map((_, i) => `0000000${i}`),
      );
      assert.equal(fs.existsSync(path.join(home, '.lock')), false);
    } finally {
      for (const root of roots) removeDir(root);
      removeDir(home);
    }
  });
});
