// Many agents at once: 16 workers in two worktrees of one clone loop `atm next --claim` / `atm done` over 40 tasks.
// Every task must be done exactly once, no claim may be granted twice, and readers polling tasks.json and the claims
// snapshot meanwhile must only ever see complete JSON.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { test } from 'node:test';
import { ATM, baseEnv, git, makeProject, removeDir, tempDir } from './helpers.mjs';

const TASKS = 40;
const WORKERS = 16;
const ids = Array.from({ length: TASKS }, (_, i) => `T-${String(i + 1).padStart(3, '0')}`);
const tasks = {
  meta: { project: 'Stress' },
  tasks: ids.map((id) => ({ id, name: id, phase: 0, type: 'feature', status: 'todo', dependencies: [] })),
};

function atm(args, cwd, agent) {
  return new Promise((resolve) => {
    const child = spawn(ATM, args, { cwd, env: { ...baseEnv(), ATM_AGENT: agent } });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (d) => (stdout += d));
    child.stderr.on('data', (d) => (stderr += d));
    // `close`, not `exit`: `exit` can come before the last of stdout is read, as it does with a fast binary under load
    child.on('close', (code) => resolve({ code, stdout, stderr }));
  });
}

async function worker(name, cwd, log) {
  for (;;) {
    const next = await atm(['loop', 'next', '--claim', '--json'], cwd, name);
    if (next.code === 3) return;
    assert.equal(next.code, 0, `${name}: ${next.stderr}`);
    const id = JSON.parse(next.stdout).task.id;
    log.push([name, id]);
    const done = await atm(['task', 'state', id, 'done'], cwd, name);
    assert.equal(done.code, 0, `${name} done ${id}: ${done.stderr}`);
  }
}

test('16 concurrent agents in two worktrees: every task done exactly once, files always valid', async () => {
  const root = makeProject({ commit: true, files: { '.agentic-task-manager/tasks.json': tasks } });
  const wt = path.join(tempDir(), 'wt2');
  git(['worktree', 'add', '-q', '-b', 'agent-2', wt], root);
  const watched = [root, wt].flatMap((dir) => [
    path.join(dir, '.agentic-task-manager/tasks.json'),
    path.join(dir, '.agentic-task-manager/claims.json'),
  ]);
  let reads = 0;
  const torn = [];
  const reader = setInterval(() => {
    for (const file of watched) {
      let text;
      try {
        text = fs.readFileSync(file, 'utf8');
      } catch {
        continue; // the snapshot doesn't exist until the first claim
      }
      reads++;
      try {
        JSON.parse(text);
      } catch {
        torn.push(`${file}: ${text.slice(0, 80)}`);
      }
    }
  }, 2);
  try {
    const log = [];
    await Promise.all(Array.from({ length: WORKERS }, (_, i) => worker(`agent-${i + 1}`, i % 2 ? wt : root, log)));
    clearInterval(reader);

    const claimed = log.map(([, id]) => id).sort();
    assert.deepEqual(claimed, ids, 'each task handed out exactly once');
    // Each task is done in exactly one worktree's tasks.json (the other learns about it when the branches merge)
    const doneIn = (dir) =>
      JSON.parse(fs.readFileSync(path.join(dir, '.agentic-task-manager/tasks.json'), 'utf8'))
        .tasks.filter((t) => t.status === 'done')
        .map((t) => t.id);
    assert.deepEqual([...doneIn(root), ...doneIn(wt)].sort(), ids);
    const store = JSON.parse(fs.readFileSync(path.join(root, '.git/atm/claims.json'), 'utf8'));
    assert.deepEqual(store.claims, {});
    assert.deepEqual(Object.keys(store.completed).sort(), ids);
    assert.deepEqual(torn, [], 'readers saw a partially written file');
    assert.ok(reads > 100, `the reader should have sampled the files often (${reads} reads)`);
  } finally {
    clearInterval(reader);
    removeDir(root);
    removeDir(path.dirname(wt));
  }
});
