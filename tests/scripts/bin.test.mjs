// plugin/bin/atm (CLI-6.1): Claude Code puts a plugin's bin/ on the Bash tool's PATH, so skills run plain `atm`.
// It must run the same atm-cli as scripts/atm, from any directory and when found through PATH.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { test } from 'node:test';
import { PLUGIN_VERSION, REPO, baseEnv, makeProject, removeDir } from './helpers.mjs';

test('bin/atm on PATH runs atm-cli as plain `atm`, from any directory', () => {
  const root = makeProject({ files: { '.agentic-task-manager/tasks.json': { tasks: [] } } });
  try {
    const env = { ...baseEnv(), PATH: `${path.join(REPO, 'plugin/bin')}${path.delimiter}${process.env.PATH}` };
    const version = spawnSync('atm', ['version'], { cwd: root, encoding: 'utf8', env });
    assert.equal(version.status, 0, version.stderr);
    assert.equal(version.stdout, `${PLUGIN_VERSION}\n`);
    const paths = spawnSync('atm', ['project', 'config', '--json'], { cwd: root, encoding: 'utf8', env });
    assert.equal(JSON.parse(paths.stdout).root, root);
  } finally {
    removeDir(root);
  }
});
