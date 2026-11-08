// The lock (atm.py `locked`): mutual exclusion between processes, breaking abandoned locks, and timing out on live
// ones. Driven through the hidden `_lock-test <seconds>` command, which holds the task folder's lock and logs
// "acquire <pid>" / "release <pid>" to .agentic-task-manager/.lock-test.log.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { describe, test } from 'node:test';
import { ATM, makeProject, removeDir, runAtm } from './helpers.mjs';

const project = () => makeProject({ files: { '.agentic-task-manager/tasks.json': { tasks: [] } } });
const lockDir = (root) => path.join(root, '.agentic-task-manager/.lock');
const log = (root) =>
  fs
    .readFileSync(path.join(root, '.agentic-task-manager/.lock-test.log'), 'utf8')
    .trim()
    .split('\n')
    .map((line) => line.split(' '));
const iso = (ms) => new Date(ms).toISOString().replace(/\.\d+Z$/, 'Z');

/** Plant a lock as if another process held it. */
function plantLock(root, owner) {
  fs.mkdirSync(lockDir(root));
  if (owner) fs.writeFileSync(path.join(lockDir(root), 'owner.json'), JSON.stringify(owner));
}

/** Start `atm _lock-test seconds` without waiting; resolves with its exit code. */
function holdLock(root, seconds, env = {}) {
  const child = spawn(ATM, ['_lock-test', String(seconds)], { cwd: root, env: { ...process.env, ...env } });
  let stderr = '';
  child.stderr.on('data', (d) => (stderr += d));
  // `close`, not `exit`: all of stderr is read by then
  const done = new Promise((resolve) => child.on('close', (code) => resolve({ code, stderr })));
  return { child, done };
}

describe('lock', () => {
  test('one holder at a time across processes, and the lock is gone afterwards', async () => {
    const root = project();
    try {
      const holders = Array.from({ length: 5 }, () => holdLock(root, 0.15));
      const results = await Promise.all(holders.map((h) => h.done));
      for (const r of results) assert.equal(r.code, 0, r.stderr);
      const lines = log(root);
      assert.equal(lines.length, 10);
      // Strictly alternating acquire/release by the same process: no two holders overlapped
      for (let i = 0; i < lines.length; i += 2) {
        assert.equal(lines[i][0], 'acquire');
        assert.deepEqual(lines[i + 1], ['release', lines[i][1]]);
      }
      assert.equal(fs.existsSync(lockDir(root)), false);
    } finally {
      removeDir(root);
    }
  });

  test('a lock left by a process that no longer runs is broken at once', () => {
    const root = project();
    try {
      plantLock(root, { host: os.hostname(), pid: 999999, acquired_at: iso(Date.now()) });
      const r = runAtm(['_lock-test', '0'], root, { ATM_LOCK_TIMEOUT: '1' });
      assert.equal(r.code, 0, r.stderr);
      assert.equal(fs.existsSync(lockDir(root)), false);
    } finally {
      removeDir(root);
    }
  });

  test('a lock older than a minute is broken, even from another host', () => {
    const root = project();
    try {
      plantLock(root, { host: 'elsewhere', pid: 1, acquired_at: iso(Date.now() - 120_000) });
      assert.equal(runAtm(['_lock-test', '0'], root, { ATM_LOCK_TIMEOUT: '1' }).code, 0);
    } finally {
      removeDir(root);
    }
  });

  test('a live lock is waited for, then times out with a clear error', () => {
    const root = project();
    try {
      // This test process is alive, so its lock is not stale
      plantLock(root, { host: os.hostname(), pid: process.pid, acquired_at: iso(Date.now()) });
      const started = Date.now();
      const r = runAtm(['_lock-test', '0'], root, { ATM_LOCK_TIMEOUT: '0.5' });
      assert.equal(r.code, 1);
      assert.match(r.stderr, new RegExp(`timed out waiting for the lock .*held by pid ${process.pid}`));
      assert.ok(Date.now() - started >= 500);
      assert.equal(fs.existsSync(lockDir(root)), true, 'a live lock is never broken');
    } finally {
      removeDir(root);
    }
  });

  test('a lock whose owner is not written yet is respected while fresh, broken when old', () => {
    const root = project();
    try {
      plantLock(root, null);
      assert.equal(runAtm(['_lock-test', '0'], root, { ATM_LOCK_TIMEOUT: '0.3' }).code, 1);
      const old = new Date(Date.now() - 120_000);
      fs.utimesSync(lockDir(root), old, old);
      assert.equal(runAtm(['_lock-test', '0'], root, { ATM_LOCK_TIMEOUT: '1' }).code, 0);
    } finally {
      removeDir(root);
    }
  });

  test('a holder stopped with SIGTERM releases the lock', async () => {
    const root = project();
    try {
      const holder = holdLock(root, 30);
      const owner = path.join(lockDir(root), 'owner.json');
      // until the holder has the lock, or has exited without it (then the assertion below says so)
      while (!fs.existsSync(owner) && holder.child.exitCode === null) await new Promise((r) => setTimeout(r, 20));
      assert.ok(fs.existsSync(owner), `the holder exited without taking the lock: ${holder.child.exitCode}`);
      holder.child.kill('SIGTERM');
      await holder.done;
      assert.equal(fs.existsSync(lockDir(root)), false);
    } finally {
      removeDir(root);
    }
  });
});
