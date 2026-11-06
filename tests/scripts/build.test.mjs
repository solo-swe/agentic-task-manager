// atm-cli/build.py: plugin/scripts/atm.py is built from the modules in atm-cli/atm/. The build must be
// deterministic, the committed file must match it, and it must refuse module layouts that concatenation would break.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { describe, test } from 'node:test';
import { REPO, removeDir, tempDir } from './helpers.mjs';

const BUILD = path.join(REPO, 'atm-cli/build.py');

function runBuild(args, build = BUILD) {
  const result = spawnSync('python3', [build, ...args], { encoding: 'utf8' });
  if (result.error) throw result.error;
  return { code: result.status, stderr: result.stderr };
}

/** A copy of atm-cli/ (and docs/agent/, which it builds in) in a temporary directory, so a test can break a module
 * without touching the repo. */
function copyOfSources() {
  const dir = tempDir('atm-build-');
  fs.cpSync(path.join(REPO, 'atm-cli'), path.join(dir, 'atm-cli'), { recursive: true });
  fs.cpSync(path.join(REPO, 'docs/agent'), path.join(dir, 'docs/agent'), { recursive: true });
  return dir;
}

describe('build', () => {
  test('the build is deterministic', () => {
    const dir = tempDir('atm-build-');
    try {
      const [a, b] = [path.join(dir, 'a.py'), path.join(dir, 'b.py')];
      assert.equal(runBuild(['--output', a]).code, 0);
      assert.equal(runBuild(['--output', b]).code, 0);
      assert.equal(fs.readFileSync(a, 'utf8'), fs.readFileSync(b, 'utf8'));
    } finally {
      removeDir(dir);
    }
  });

  test('the committed plugin/scripts/atm.py is what the modules build', () => {
    const { code, stderr } = runBuild(['--check']);
    assert.equal(code, 0, stderr);
  });

  test('a name defined in two modules is refused', () => {
    const dir = copyOfSources();
    try {
      fs.appendFileSync(path.join(dir, 'atm-cli/atm/lib/project.py'), '\n\ndef out(text=""):\n    pass\n');
      const { code, stderr } = runBuild(['--output', path.join(dir, 'atm.py')], path.join(dir, 'atm-cli/build.py'));
      assert.equal(code, 1);
      assert.match(stderr, /out is defined in both common\/core\.py and lib\/project\.py/);
    } finally {
      removeDir(dir);
    }
  });

  test('importing from a later module, or a name a module does not define, is refused', () => {
    const dir = copyOfSources();
    try {
      const errors = path.join(dir, 'atm-cli/atm/common/errors.py');
      const original = fs.readFileSync(errors, 'utf8');
      const output = path.join(dir, 'atm.py');
      const build = path.join(dir, 'atm-cli/build.py');

      fs.writeFileSync(errors, original + '\nfrom .core import now\n');
      assert.match(
        runBuild(['--output', output], build).stderr,
        /common\/errors\.py imports from common\.core, which isn't an earlier module/,
      );

      fs.writeFileSync(errors, original);
      fs.appendFileSync(path.join(dir, 'atm-cli/atm/lib/project.py'), '\nfrom ..common.core import no_such_name\n');
      assert.match(runBuild(['--output', output], build).stderr, /imports no_such_name from common\.core/);
    } finally {
      removeDir(dir);
    }
  });

  test('the built file ends in the one entry: --server is the REST API’s server, anything else the CLI', () => {
    const built = fs.readFileSync(path.join(REPO, 'plugin/scripts/atm.py'), 'utf8');
    assert.match(
      built,
      /if __name__ == "__main__":\n {4}if sys\.argv\[1:2\] == \[SERVER_ENTRY\]:\n {8}sys\.exit\(serve_main\(sys\.argv\[2:\]\)\)\n {4}sys\.exit\(main\(sys\.argv\[1:\]\)\)\n$/,
    );
    const launcher = fs.readFileSync(path.join(REPO, 'plugin/scripts/atm-serve'), 'utf8');
    assert.match(launcher, /exec python3 "\$dir\/atm\.py" --server "\$@"/);
    assert.ok(fs.statSync(path.join(REPO, 'plugin/scripts/atm-serve')).mode & 0o111, 'atm-serve is executable');
  });
});

// The types generated from common's schemas (docs/layered-architecture.md §4): Python and TypeScript, one generator
describe('generated types', () => {
  test('a schema changed without rebuilding fails the check', () => {
    const dir = copyOfSources();
    try {
      const file = path.join(dir, 'atm-cli/atm/common/schema/task.schema.json');
      const schema = JSON.parse(fs.readFileSync(file, 'utf8'));
      schema.properties.estimate = { type: 'integer' };
      fs.writeFileSync(file, JSON.stringify(schema));
      const { code, stderr } = runBuild(['--check'], path.join(dir, 'atm-cli/build.py'));
      assert.equal(code, 1);
      assert.match(stderr, /atm-cli\/atm\/common\/types\.py is stale/);
    } finally {
      removeDir(dir);
    }
  });

  test('both languages get every field, required or not', () => {
    const python = fs.readFileSync(path.join(REPO, 'atm-cli/atm/common/types.py'), 'utf8');
    const typescript = fs.readFileSync(path.join(REPO, 'dashboard/src/lib/generated/types.ts'), 'utf8');
    assert.match(python, /# required; Letters, digits.*\n {8}"id": str,/);
    assert.match(python, /"from": str,/); // a Python keyword, as a key
    assert.match(python, /"notes": List\["Note"\],/); // a forward reference: defined anywhere in the file
    assert.match(typescript, /\n {2}id: string;/);
    assert.match(typescript, /\n {2}notes\?: Note\[\];/);
    assert.match(typescript, /\n {2}pr\?: PullRequest;/);
  });

  test('a field that may be null, and a map, in both languages (T-382)', () => {
    const python = fs.readFileSync(path.join(REPO, 'atm-cli/atm/common/types.py'), 'utf8');
    const typescript = fs.readFileSync(path.join(REPO, 'dashboard/src/lib/generated/types.ts'), 'utf8');
    assert.match(python, /"active_sprint": Optional\["ActiveSprint"\],/); // anyOf with null
    assert.match(python, /"recent_sprints": Optional\[List\["SprintNumbers"\]\],/); // a list of types with null
    assert.match(python, /"by_status": Dict\[str, int\],/);
    assert.match(python, /^from typing import Dict, List, Literal, Optional, TypedDict$/m);
    assert.match(typescript, /\n {2}active_sprint: ActiveSprint \| null;/);
    assert.match(typescript, /\n {2}recent_sprints: SprintNumbers\[\] \| null;/);
    assert.match(typescript, /\n {2}by_status: Record<string, number>;/);
  });

  test('a list of types that is not one type and null is refused', () => {
    const dir = copyOfSources();
    try {
      const file = path.join(dir, 'atm-cli/atm/common/schema/task.schema.json');
      const schema = JSON.parse(fs.readFileSync(file, 'utf8'));
      schema.properties.estimate = { type: ['integer', 'string'] };
      fs.writeFileSync(file, JSON.stringify(schema));
      const { code, stderr } = runBuild(['--output', path.join(dir, 'atm.py')], path.join(dir, 'atm-cli/build.py'));
      assert.equal(code, 1);
      assert.match(stderr, /Task\.estimate: a list of types is one type and null/);
    } finally {
      removeDir(dir);
    }
  });
});
