// The files `--*-file` options name (ATM-662): read from anywhere, a scratch folder outside the project too (the skills
// put them there), but never a key or environment file, one in a credentials folder (also through a symbolic link),
// anything but a regular file, or anything over 1 MiB; each refused as a usage error, exit code 2, with nothing written.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { describe, test } from 'node:test';
import { makeProject, removeDir, runAtm, tempDir } from './helpers.mjs';

const TASKS = '.agentic-task-manager/tasks.json';

describe('--*-file options (ATM-662)', () => {
  const setup = () =>
    makeProject({
      git: true,
      commit: true,
      files: { [TASKS]: { tasks: [{ id: 'T-1', name: 'One', type: 'feature', status: 'todo', description: 'old' }] } },
    });
  const description = (root) => JSON.parse(fs.readFileSync(path.join(root, TASKS), 'utf8')).tasks[0].description;

  test('a file in the project or in a scratch folder outside it is read', () => {
    const root = setup();
    const scratch = tempDir('atm-scratch-');
    try {
      fs.writeFileSync(path.join(root, 'desc.md'), 'From the project.\n');
      assert.equal(runAtm(['task', 'edit', 'T-1', '--description-file', 'desc.md'], root).code, 0);
      assert.equal(description(root), 'From the project.\n');
      const outside = path.join(scratch, 'desc.md');
      fs.writeFileSync(outside, 'From a scratch folder.\n');
      const r = runAtm(['task', 'edit', 'T-1', '--description-file', outside], root);
      assert.equal(r.code, 0, r.stderr);
      assert.equal(description(root), 'From a scratch folder.\n');
    } finally {
      removeDir(scratch);
      removeDir(root);
    }
  });

  test('a key file, a credentials folder, a link to one, a device, a folder or over 1 MiB: refused, exit 2, nothing written', () => {
    const root = setup();
    const home = tempDir('atm-home-');
    try {
      fs.mkdirSync(path.join(home, '.ssh'));
      fs.writeFileSync(path.join(home, '.ssh/id_ed25519'), '-----BEGIN OPENSSH PRIVATE KEY-----\n');
      fs.writeFileSync(path.join(home, '.ssh/config'), 'Host x\n');
      fs.writeFileSync(path.join(root, 'server.pem'), 'key\n');
      fs.writeFileSync(path.join(root, '.env'), 'TOKEN=x\n');
      fs.writeFileSync(path.join(root, 'big.md'), 'x'.repeat(2 * 1024 * 1024));
      fs.mkdirSync(path.join(root, 'folder'));
      const cases = [
        [path.join(home, '.ssh/id_ed25519'), /key or environment/],
        [path.join(home, '.ssh/config'), /credentials folder/],
        ['server.pem', /key or environment/],
        ['.env', /key or environment/],
        ['big.md', /larger than 1 MiB/],
        ['folder', /not a regular file/],
        ['gone.md', /can't read it/],
      ];
      if (process.platform !== 'win32') {
        fs.symlinkSync(path.join(home, '.ssh/id_ed25519'), path.join(root, 'notes.md'));
        cases.push(['notes.md', /key or environment/], ['/dev/zero', /not a regular file/]);
      }
      for (const [file, says] of cases) {
        const r = runAtm(['task', 'edit', 'T-1', '--description-file', file], root);
        assert.equal(r.code, 2, `${file}: ${r.stderr}`);
        assert.match(r.stderr, says, file);
        assert.match(r.stderr, /--description-file /, file);
      }
      assert.equal(description(root), 'old', 'nothing written');
      // Every option reads through the same door
      for (const args of [
        ['task', 'add', '--json-file', 'server.pem'],
        ['task', 'add', '--name', 'x', '--type', 'feature', '--description-file', 'server.pem'],
        ['db', 'init', '--meta-file', 'server.pem'],
      ]) {
        const r = runAtm(args, root);
        assert.equal(r.code, 2, `${args.join(' ')}: ${r.stderr}`);
        assert.match(r.stderr, /key or environment/, args.join(' '));
      }
    } finally {
      removeDir(home);
      removeDir(root);
    }
  });
});
