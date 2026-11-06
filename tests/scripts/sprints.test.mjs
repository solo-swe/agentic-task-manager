// Generated sprint names (docs/sprints-and-releases-design.md SR-2.9): what scenarios can't say with one fixed
// output, that a generated name is never one already in use.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { test } from 'node:test';
import { makeProject, removeDir, runAtm } from './helpers.mjs';

const FILE = '.agentic-task-manager/tasks.json';

test('generated people names: each new sprint gets a name not used yet, in either file, in any case', () => {
  const root = makeProject({
    files: {
      [FILE]: { meta: { sprints: [{ id: 'ada lovelace' }] }, tasks: [] },
      '.agentic-task-manager/tasks-archive.json': {
        meta: {},
        tasks: [
          { id: 'T-1', status: 'done', sprint_history: [{ sprint: 'Alan Turing', from: '2026-01-01T00:00:00Z' }] },
        ],
      },
    },
  });
  try {
    const names = [];
    // The same seed every time: only the names already taken change what is picked
    for (let i = 0; i < 5; i++) {
      const r = runAtm(['sprint', 'add', '--json'], root, { ATM_SEED: '7' });
      assert.equal(r.code, 0, r.stderr);
      names.push(JSON.parse(r.stdout).id);
    }
    const lower = names.map((n) => n.toLowerCase());
    assert.equal(new Set(lower).size, 5, `distinct: ${names}`);
    assert.ok(!lower.includes('ada lovelace') && !lower.includes('alan turing'), `not taken: ${names}`);
    for (const n of names) assert.match(n, /^[A-Za-z][A-Za-z .-]*$/, 'plain ASCII, no apostrophes');
    const stored = JSON.parse(fs.readFileSync(path.join(root, FILE), 'utf8')).meta.sprints.map((s) => s.id);
    assert.deepEqual(stored, ['ada lovelace', ...names]);
  } finally {
    removeDir(root);
  }
});
