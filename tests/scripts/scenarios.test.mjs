// Data-driven scenarios (scenarios/*.json): set up a project, run one atm command, check its output and files. A
// scenario is:
// {
//   "name": "...",
//   "setup": { "files": {...}, "git": true, "commit": true, "commands": [[...]] },   see makeProject()
//   "cwd": "docs",                          optional, relative to the project root
//   "args": ["paths", "--json"],
//   "env": { "ATM_NOW": "2026-01-01T00:00:00Z" },   optional extra environment (ATM_HOME defaults to {root}/.atm-home)
//   "expect": {
//     "exit": 0,
//     "stdout": "exact text", "json": <value parsed from stdout>, "stdoutIncludes": ["..."], "stderrIncludes": ["..."],
//     "files": { "path": "exact text" | <JSON value> | true (exists) | false (absent) },
//     "git": { "staged": ["path", ...] }    `git diff --cached --name-status` lines, e.g. "R docs/a .agentic-task-manager/a"
//   }
// }
// Strings in args, env and expect may use {root} (the project's real path), {version} (the plugin version) and
// {build} (the shipped dashboard's build ID); setup only {version} and {build}.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { describe, test } from 'node:test';
import { PLUGIN_BUILD, PLUGIN_VERSION, REPO, fill, git, makeProject, removeDir, runAtm } from './helpers.mjs';

const dir = path.join(REPO, 'tests/scripts/scenarios');
const scenarios = fs
  .readdirSync(dir)
  .filter((f) => f.endsWith('.json'))
  .sort()
  .flatMap((f) => {
    const data = JSON.parse(fs.readFileSync(path.join(dir, f), 'utf8'));
    return (Array.isArray(data) ? data : [data]).map((s) => ({
      ...s,
      file: f,
    }));
  });

function checkFile(root, file, expected) {
  const full = path.join(root, file);
  if (expected === false) return assert.ok(!fs.existsSync(full), `${file} should not exist`);
  assert.ok(fs.existsSync(full), `${file} should exist`);
  if (expected === true) return;
  const text = fs.readFileSync(full, 'utf8');
  if (typeof expected === 'string') assert.equal(text, expected, file);
  else assert.deepEqual(JSON.parse(text), expected, file);
}

describe('scenarios', () => {
  for (const s of scenarios) {
    test(`${s.file}: ${s.name}`, () => {
      const root = makeProject(fill(s.setup, { root: '{root}', version: PLUGIN_VERSION, build: PLUGIN_BUILD }));
      try {
        const vars = { root, version: PLUGIN_VERSION, build: PLUGIN_BUILD };
        const e = fill(s.expect, vars);
        // Each scenario has its own ATM_HOME, .atm-home in the project, so `expect.files` can check the registry
        const env = { ATM_HOME: path.join(root, '.atm-home'), ...fill(s.env || {}, vars) };
        const r = runAtm(fill(s.args, vars), path.join(root, s.cwd || '.'), env);
        const context = `stdout:\n${r.stdout}\nstderr:\n${r.stderr}`;
        assert.equal(r.code, e.exit ?? 0, `exit code\n${context}`);
        if ('stdout' in e) assert.equal(r.stdout, e.stdout, context);
        if ('json' in e) assert.deepEqual(JSON.parse(r.stdout), e.json, context);
        for (const s of e.stdoutIncludes || [])
          assert.ok(r.stdout.includes(s), `stdout should include "${s}"\n${context}`);
        for (const s of e.stderrIncludes || [])
          assert.ok(r.stderr.includes(s), `stderr should include "${s}"\n${context}`);
        for (const [file, expected] of Object.entries(e.files || {})) checkFile(root, file, expected);
        if (e.git?.staged) {
          const staged = git(['diff', '--cached', '--name-status', '-M'], root)
            .split('\n')
            .filter(Boolean)
            .map((l) => l.replace(/^R\d+/, 'R').replace(/\t/g, ' '))
            .sort();
          assert.deepEqual(staged, [...e.git.staged].sort());
        }
      } finally {
        removeDir(root);
      }
    });
  }
});
