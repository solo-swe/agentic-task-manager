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

/** A copy of atm-cli/ in a temporary directory, so a test can break a module without touching the repo. */
function copyOfSources() {
  const dir = tempDir('atm-build-');
  fs.cpSync(path.join(REPO, 'atm-cli'), path.join(dir, 'atm-cli'), { recursive: true });
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
      fs.appendFileSync(path.join(dir, 'atm-cli/atm/paths.py'), '\n\ndef out(text=""):\n    pass\n');
      const { code, stderr } = runBuild(['--output', path.join(dir, 'atm.py')], path.join(dir, 'atm-cli/build.py'));
      assert.equal(code, 1);
      assert.match(stderr, /out is defined in both core\.py and paths\.py/);
    } finally {
      removeDir(dir);
    }
  });

  test('importing from a later module, or a name a module does not define, is refused', () => {
    const dir = copyOfSources();
    try {
      const core = path.join(dir, 'atm-cli/atm/core.py');
      const original = fs.readFileSync(core, 'utf8');
      const output = path.join(dir, 'atm.py');
      const build = path.join(dir, 'atm-cli/build.py');

      fs.writeFileSync(core, original + '\nfrom .paths import find_paths\n');
      assert.match(runBuild(['--output', output], build).stderr, /core\.py imports from paths/);

      fs.writeFileSync(core, original);
      fs.appendFileSync(path.join(dir, 'atm-cli/atm/paths.py'), '\nfrom .core import no_such_name\n');
      assert.match(runBuild(['--output', output], build).stderr, /imports no_such_name from core/);
    } finally {
      removeDir(dir);
    }
  });
});

// The layer rule and its ratchet (docs/layered-architecture.md §1, §8, §9): atm-cli/layers.json lists what still
// breaks the rule, and only shrinks
describe('layers', () => {
  function brokenBuild(change) {
    const dir = copyOfSources();
    try {
      change(path.join(dir, 'atm-cli'));
      return runBuild(['--output', path.join(dir, 'atm.py')], path.join(dir, 'atm-cli/build.py'));
    } finally {
      removeDir(dir);
    }
  }

  test('atm-lib may not import a store, only its interface', () => {
    const { code, stderr } = brokenBuild((cli) =>
      fs.appendFileSync(path.join(cli, 'atm/lib/tasks.py'), '\nfrom ..db.jsongit.store import JsonGitStore\n'),
    );
    assert.equal(code, 1);
    assert.match(stderr, /lib\/tasks\.py imports from db\.jsongit\.store, which the lib layer may not use/);
  });

  test('the CLI may not reach the store', () => {
    const { stderr } = brokenBuild((cli) =>
      fs.appendFileSync(path.join(cli, 'atm/cli/show.py'), '\nfrom ..db.memory import MemoryStore\n'),
    );
    assert.match(stderr, /cli\/show\.py imports from db\.memory, which the cli layer may not use/);
  });

  test('a new import from a module before the layers needs an entry in layers.json', () => {
    const { stderr } = brokenBuild((cli) =>
      fs.appendFileSync(path.join(cli, 'atm/lib/tasks.py'), '\nfrom ..model import kind_of\n'),
    );
    assert.match(stderr, /lib\/tasks\.py imports from model/);
  });

  test('a new module outside the layers is refused', () => {
    const { stderr } = brokenBuild((cli) => {
      fs.writeFileSync(path.join(cli, 'atm/extra.py'), '"""An extra module."""\n');
      const build = path.join(cli, 'build.py');
      fs.writeFileSync(
        build,
        fs.readFileSync(build, 'utf8').replace('    "cli.main",\n', '    "extra",\n    "cli.main",\n'),
      );
    });
    assert.match(stderr, /extra\.py is a new module outside the layers/);
  });

  test('an entry no longer needed must be removed, so the list only shrinks', () => {
    const { stderr } = brokenBuild((cli) => {
      const file = path.join(cli, 'layers.json');
      const layers = JSON.parse(fs.readFileSync(file, 'utf8'));
      layers.exceptions['lib.project'] = ['model'];
      fs.writeFileSync(file, JSON.stringify(layers));
    });
    assert.match(stderr, /allows lib\.project to import from model, which it no longer does/);
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
});
