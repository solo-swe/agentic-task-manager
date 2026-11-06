// A cloned repository can commit symlinks. atm never reads or writes the task folder through one, so a link can't
// make it append to ~/.zshrc or copy a private file into tasks.json (docs/security-review.md F7, T-224).
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { describe, test } from 'node:test';
import { makeProject, removeDir, runAtm } from './helpers.mjs';

const TASKS = { meta: { id: 'abcd1234', project: 'Linked' }, tasks: [] };
const outside = () => fs.mkdtempSync(path.join(os.tmpdir(), 'atm-outside-'));

describe('symlinks in the task folder', () => {
  for (const name of ['.gitignore', '.gitattributes', 'tasks-archive.json', 'tasks.json']) {
    test(`a symlinked ${name}: atm refuses, and the file it points to is untouched`, () => {
      const elsewhere = outside();
      const target = path.join(elsewhere, 'target');
      fs.writeFileSync(target, 'export PATH=/usr/bin\n');
      const root = makeProject({
        git: true,
        files: { '.agentic-task-manager/tasks.json': TASKS },
      });
      try {
        const link = path.join(root, '.agentic-task-manager', name);
        fs.rmSync(link, { force: true });
        fs.symlinkSync(target, link);
        for (const args of [
          ['db', 'migrate'],
          ['db', 'init'],
          ['project', 'status'],
        ]) {
          const r = runAtm(args, root, { ATM_HOME: path.join(root, '.atm-home') });
          assert.equal(r.code, 1, `${args}: ${r.stdout}${r.stderr}`);
          assert.match(r.stderr, /is a symlink/);
        }
        assert.equal(fs.readFileSync(target, 'utf8'), 'export PATH=/usr/bin\n');
      } finally {
        removeDir(root);
        removeDir(elsewhere);
      }
    });
  }

  test('a task folder that is a symlink to elsewhere: atm refuses', () => {
    const elsewhere = outside();
    fs.writeFileSync(path.join(elsewhere, 'tasks.json'), JSON.stringify(TASKS));
    const root = makeProject({ git: true, files: { 'README.md': 'x\n' } });
    try {
      fs.symlinkSync(elsewhere, path.join(root, '.agentic-task-manager'));
      const r = runAtm(['db', 'init'], root, { ATM_HOME: path.join(root, '.atm-home') });
      assert.equal(r.code, 1, r.stdout + r.stderr);
      assert.match(r.stderr, /is a symlink/);
      assert.deepEqual(fs.readdirSync(elsewhere), ['tasks.json']); // no .gitignore or .gitattributes written there
    } finally {
      removeDir(root);
      removeDir(elsewhere);
    }
  });
});
