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
});

// The layer rule (docs/layered-architecture.md §1, §8), with no exceptions since the migration's last step
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

  test('the CLI and the REST API never import each other: the built file’s entry puts them together', () => {
    const { stderr } = brokenBuild((cli) =>
      fs.appendFileSync(path.join(cli, 'atm/cli/show.py'), '\nfrom ..api.server import PORT_TRIES\n'),
    );
    assert.match(stderr, /cli\/show\.py imports from api\.server, which the cli layer may not use/);
    const back = brokenBuild((cli) =>
      fs.appendFileSync(path.join(cli, 'atm/api/server.py'), '\nfrom ..cli.render import report_expired\n'),
    );
    assert.match(back.stderr, /api\/server\.py imports from cli\.render, which the api layer may not use/);
  });

  test("the CLI and the REST API never reach a project's store: only atm-lib does", () => {
    const peek = '\n\ndef peek(project):\n    return project.store.document()\n';
    const { code, stderr } = brokenBuild((cli) => fs.appendFileSync(path.join(cli, 'atm/cli/show.py'), peek));
    assert.equal(code, 1);
    assert.match(stderr, /cli\/show\.py:\d+ reaches a project's store \(\.store\): the cli layer reads and changes/);
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

  test('only the stores and the version control run git: the host layer is theirs (T-385)', () => {
    const lib = brokenBuild((cli) =>
      fs.appendFileSync(path.join(cli, 'atm/lib/code.py'), '\nfrom ..host.git import git\n'),
    );
    assert.match(lib.stderr, /lib\/code\.py imports from host\.git, which the lib layer may not use/);
    const boot = brokenBuild((cli) =>
      fs.appendFileSync(path.join(cli, 'atm/boot/open.py'), '\nfrom ..host.git import in_git\n'),
    );
    assert.match(boot.stderr, /boot\/open\.py imports from host\.git, which the boot layer may not use/);
    const below = brokenBuild((cli) =>
      fs.appendFileSync(path.join(cli, 'atm/host/git.py'), '\nfrom ..db.interface import Store\n'),
    );
    assert.match(below.stderr, /host\/git\.py imports from db\.interface, which the host layer may not use/);
  });

  test('a module outside the layers is refused', () => {
    const { stderr } = brokenBuild((cli) => {
      fs.writeFileSync(path.join(cli, 'atm/extra.py'), '"""An extra module."""\n');
      const build = path.join(cli, 'build.py');
      fs.writeFileSync(
        build,
        fs.readFileSync(build, 'utf8').replace('    "cli.main",\n', '    "extra",\n    "cli.main",\n'),
      );
    });
    assert.match(stderr, /extra\.py is a module outside the layers/);
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
