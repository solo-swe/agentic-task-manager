// Several agents adding tasks at once (DM-2.3): meta.next_id is read and advanced under the lock, so every new task
// gets its own ID and the file stays valid.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { test } from 'node:test';
import { ATM, baseEnv, makeProject, removeDir } from './helpers.mjs';

const AGENTS = 8;
const EACH = 3;

function atm(args, cwd, agent) {
  return new Promise((resolve) => {
    const child = spawn(ATM, args, { cwd, env: { ...baseEnv(), ATM_AGENT: agent } });
    let stderr = '';
    child.stderr.on('data', (d) => (stderr += d));
    // `close`, not `exit`: all of stderr is read by then
    child.on('close', (code) => resolve({ code, stderr }));
  });
}

test(`${AGENTS} agents adding ${EACH} tasks each at once: every task gets its own ID`, async () => {
  const root = makeProject({ files: { '.agentic-task-manager/tasks.json': { tasks: [] } } });
  try {
    await Promise.all(
      Array.from({ length: AGENTS }, async (_, a) => {
        for (let i = 0; i < EACH; i++) {
          const r = await atm(['task', 'add', '--name', `a${a}-${i}`, '--type', 'docs'], root, `agent-${a}`);
          assert.equal(r.code, 0, r.stderr);
        }
      }),
    );
    const data = JSON.parse(fs.readFileSync(path.join(root, '.agentic-task-manager/tasks.json'), 'utf8'));
    const ids = data.tasks.map((t) => t.id);
    assert.equal(ids.length, AGENTS * EACH);
    assert.equal(new Set(ids).size, ids.length, 'no ID given twice');
    assert.equal(data.meta.next_id, AGENTS * EACH + 1);
    assert.deepEqual(
      [...ids].sort(),
      Array.from({ length: AGENTS * EACH }, (_, i) => `T-${String(i + 1).padStart(3, '0')}`),
    );
  } finally {
    removeDir(root);
  }
});
