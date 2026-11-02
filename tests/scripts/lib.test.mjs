// atm-lib's unit tests and the store contract (tests/lib/, Python's unittest): run here so the suite, and CI with
// both Python versions, include them (docs/layered-architecture.md §9).
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { test } from 'node:test';
import { REPO } from './helpers.mjs';

test('atm-lib unit tests (tests/lib)', () => {
  const dir = path.join(REPO, 'tests/lib');
  const result = spawnSync('python3', ['-m', 'unittest', 'discover', '-s', dir, '-t', dir], {
    cwd: REPO,
    encoding: 'utf8',
  });
  if (result.error) throw result.error;
  assert.equal(result.status, 0, result.stderr);
});
