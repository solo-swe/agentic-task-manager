// The libraries copied into plugin/scripts/vendor/ (T-425) are the versions atm-cli/uv.lock pins, and the copy leaves
// out Typer's rich support, so help looks the same on every machine (plugin/scripts/vendor/README.md).
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { test } from 'node:test';
import { REPO } from './helpers.mjs';

const VENDOR = path.join(REPO, 'plugin/scripts/vendor');
const lock = fs.readFileSync(path.join(REPO, 'atm-cli/uv.lock'), 'utf8');

function locked(name) {
  return new RegExp(`^name = "${name}"\\nversion = "([^"]+)"`, 'm').exec(lock)?.[1];
}

function vendored(name) {
  const init = fs.readFileSync(path.join(VENDOR, name, '__init__.py'), 'utf8');
  return /^__version__ = "([^"]+)"/m.exec(init)?.[1];
}

for (const name of ['click', 'typer']) {
  test(`the vendored ${name} is the version uv.lock pins`, () => {
    assert.ok(locked(name), `uv.lock has no ${name}`);
    assert.equal(vendored(name), locked(name));
  });
}

test("Typer's rich support isn't vendored", () => {
  assert.ok(!fs.existsSync(path.join(VENDOR, 'typer/rich_utils.py')));
});
