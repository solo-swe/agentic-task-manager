// ATM-727 (Hakan): review watches burned CPU while they waited, each 8–12% of a core for hours. Every five-second
// recheck looked in full (every review read, the tasks, git's working copy) though nothing had changed, and a watch
// whose shell was gone ran on until its time was up, hours later.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { describe, test } from 'node:test';
import { ATM, baseEnv, makeProject, removeDir, tempDir } from '../helpers.mjs';
import { AGENT, HAKAN, background, ok, project, until } from './fixtures.mjs';

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

/** A watch's cursor (written on every look, ATM-624), in the state folder `home`; none before the first look */
function cursorOf(home) {
  const dir = path.join(home, 'watches');
  if (!fs.existsSync(dir)) return undefined;
  const name = fs
    .readdirSync(dir)
    .find((f) => /^[0-9a-f]{16}\.json$/.test(f));
  return name && path.join(dir, name);
}

const alive = (pid) => {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};

describe('a review watch while it waits (ATM-727)', () => {
  test('a recheck with nothing changed doesn’t look again; a change still is heard at once', async () => {
    const root = project();
    const home = tempDir('atm-watch-home-');
    const env = { ...AGENT, ATM_HOME: home };
    let watching;
    try {
      // Its own review, so a decision on it is the agent's to hear
      fs.writeFileSync(path.join(root, 'a.txt'), 'changed\n');
      ok(['review', 'create', '--title', 'Quiet', '--task', 'T-1', '--uncommitted'], root, env);
      watching = background(['review', 'watch', '--json', '--timeout', '30s'], root, env);
      await until(() => /Watching the reviews/.test(watching.stderr()), 'the watch to start');
      await until(() => cursorOf(home) !== undefined, 'the first look');
      // What was written just before the watch began may still be told late (FSEvents): a look or two, then quiet
      let first = fs.statSync(cursorOf(home)).mtimeMs;
      for (let settled = 0; settled < 3; ) {
        await sleep(500);
        const now = fs.statSync(cursorOf(home)).mtimeMs;
        settled = now === first ? settled + 1 : 0;
        first = now;
      }
      // Past a recheck (every 5 s), nothing changed: no look, so the cursor isn't written again
      await sleep(6_500);
      assert.equal(fs.statSync(cursorOf(home)).mtimeMs, first, 'looked again with nothing changed');
      // A decision on its own review is still told, from the store's notice, well within a recheck
      const asked = Date.now();
      ok(['review', 'submit', 'CR-1', '--request-changes'], root, { ...HAKAN, ATM_HOME: home });
      await until(() => watching.lines.length >= 1, 'the decision heard', 10_000);
      assert.equal(JSON.parse(watching.lines[0]).decision, 'changes_requested');
      assert.ok(Date.now() - asked < 4_000, `heard after ${Date.now() - asked} ms`);
    } finally {
      watching?.stop();
      removeDir(root);
      removeDir(home);
    }
  });

  // Hakan on CR-125: no timed looks at all; what a look reads is watched instead
  test('a running watch hears at once what becomes the agent’s when it takes the task (the task file watched)', async () => {
    const root = makeProject({
      commit: true,
      files: {
        '.agentic-task-manager/tasks.json': {
          meta: { project: 'Takes' },
          tasks: [{ id: 'T-1', name: 'One', type: 'feature', status: 'todo', files: ['a.txt'] }],
        },
        'a.txt': 'a\n',
      },
    });
    const home = tempDir('atm-watch-home-');
    const A = { ATM_AGENT: 'claude:A', ATM_HOME: home };
    const B = { ATM_AGENT: 'claude:B', ATM_HOME: home };
    let watching;
    try {
      // B worked T-1 and had it sent back; nothing of it is A's
      ok(['task', 'state', 'T-1', 'in_progress'], root, B);
      fs.writeFileSync(path.join(root, 'a.txt'), 'a, changed\n');
      ok(['review', 'create', '--title', 'One', '--task', 'T-1', '--uncommitted'], root, B);
      ok(['review', 'comment', 'CR-1', '--file', 'a.txt', '--lines', '1', '--body', 'Name it better.'], root, { ...HAKAN, ATM_HOME: home });
      ok(['review', 'submit', 'CR-1', '--request-changes'], root, { ...HAKAN, ATM_HOME: home });
      watching = background(['review', 'watch', '--json', '--timeout', '30s', '--since', '2000-01-01T00:00:00Z'], root, A);
      await until(() => /Watching the reviews/.test(watching.stderr()), 'the watch to start');
      await until(() => cursorOf(home) !== undefined, 'the first look');
      await sleep(1_000);
      assert.equal(watching.lines.length, 0, 'not A’s work');
      // A takes the task back: the task file changes, and the watch tells both at once
      const taken = Date.now();
      ok(['task', 'state', 'T-1', 'in_progress'], root, A);
      await until(() => watching.lines.length >= 2, 'both told', 10_000);
      assert.ok(Date.now() - taken < 4_000, `told after ${Date.now() - taken} ms`);
    } finally {
      watching?.stop();
      removeDir(root);
      removeDir(home);
    }
  });

  test('the working copy wakes a watch only while a live review is open', async () => {
    const root = project();
    const home = tempDir('atm-watch-home-');
    const env = { ...AGENT, ATM_HOME: home };
    let watching;
    /** The cursor's time once it has stopped moving: what was written before may be told late (FSEvents) */
    const settled = async () => {
      let last = fs.statSync(cursorOf(home)).mtimeMs;
      for (let quiet = 0; quiet < 3; ) {
        await sleep(500);
        const now = fs.statSync(cursorOf(home)).mtimeMs;
        quiet = now === last ? quiet + 1 : 0;
        last = now;
      }
      return last;
    };
    try {
      watching = background(['review', 'watch', '--json', '--timeout', '40s'], root, env);
      await until(() => /Watching the reviews/.test(watching.stderr()), 'the watch to start');
      await until(() => cursorOf(home) !== undefined, 'the first look');
      // No live review: an edit there is nothing to the watch
      let before = await settled();
      fs.writeFileSync(path.join(root, 'a.txt'), 'edited\n');
      await sleep(2_000);
      assert.equal(fs.statSync(cursorOf(home)).mtimeMs, before, 'woken by the working copy without a live review');
      // A live review opened: the next edit there wakes it
      ok(['review', 'live'], root, env);
      before = await settled();
      fs.writeFileSync(path.join(root, 'a.txt'), 'edited again\n');
      await until(() => fs.statSync(cursorOf(home)).mtimeMs !== before, 'the edit to wake the watch', 4_000);
    } finally {
      watching?.stop();
      removeDir(root);
      removeDir(home);
    }
  });

  // A harness stopping a watch stops its shell; the watch, given to the system, ran on unread
  test('a watch whose shell is gone stops', { skip: process.platform === 'win32' }, async () => {
    const root = project();
    const home = tempDir('atm-watch-home-');
    try {
      // The shell starts the watch and, once it has started, ends, as a stopped harness's shell does
      const shell = spawn('sh', ['-c', `"$ATM" review watch --json --timeout 60s >/dev/null 2>&1 & echo $!; sleep 2`], {
        cwd: root,
        env: { ...baseEnv(), ...AGENT, ATM_HOME: home, ATM },
      });
      let out = '';
      shell.stdout.on('data', (d) => (out += d));
      await new Promise((resolve) => shell.on('close', resolve));
      const pid = Number(out.trim());
      assert.ok(pid > 0, `the watch's pid: ${out}`);
      // It notices at its next wake, within a recheck (5 s)
      await until(() => !alive(pid), 'the orphaned watch to stop', 15_000);
    } finally {
      removeDir(root);
      removeDir(home);
    }
  });
});
