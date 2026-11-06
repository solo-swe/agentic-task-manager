// Notes stay short (DM-4.2): at most 500 characters, anything longer belongs in a file in the repository. The other
// note behaviour is in scenarios/notes.json; exact lengths are easier to write here.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { describe, test } from 'node:test';
import { makeProject, removeDir, runAtm } from './helpers.mjs';

const tasks = (root) => JSON.parse(fs.readFileSync(path.join(root, '.agentic-task-manager/tasks.json'), 'utf8'));

describe('note length', () => {
  test('500 characters are fine; 501 are refused with a hint, and nothing changes', () => {
    const root = makeProject({
      files: { '.agentic-task-manager/tasks.json': { tasks: [{ id: 'T-1', status: 'in_progress' }] } },
    });
    try {
      const env = { ATM_AGENT: 'alice' };
      const tooLong = runAtm(['task', 'state', 'T-1', 'todo', '--note', 'x'.repeat(501)], root, env);
      assert.equal(tooLong.code, 2);
      assert.match(tooLong.stderr, /a note is at most 500 characters \(this one has 501\); put longer text in a file/);
      assert.deepEqual(tasks(root), { tasks: [{ id: 'T-1', status: 'in_progress' }] });

      assert.equal(runAtm(['task', 'note', 'T-1', 'y'.repeat(501)], root, env).code, 2);
      assert.equal(runAtm(['task', 'note', 'T-1', 'y'.repeat(500)], root, env).code, 0);
      assert.equal(tasks(root).tasks[0].notes[0].text.length, 500);
    } finally {
      removeDir(root);
    }
  });
});
