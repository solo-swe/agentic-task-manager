// Shared helpers for the atm script tests. Tests run the helper through its launcher (plugin/scripts/atm), as the
// skills do.
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
/** Which atm the suite runs (T-443): the Python one through its launcher, or with ATM_IMPL=rust the Rust binary
 * (`cargo build` first; ATM_RUST_BIN names another build, e.g. a release one) */
export const IMPL = process.env.ATM_IMPL === 'rust' ? 'rust' : 'python';
export const ATM =
  IMPL === 'rust'
    ? process.env.ATM_RUST_BIN || path.join(REPO, 'target/debug', process.platform === 'win32' ? 'atm.exe' : 'atm')
    : path.join(REPO, 'plugin/scripts/atm');
/** atm's version: the Rust workspace's (Cargo.toml's [workspace.package]; plugin.json says the same until T-454) */
export const PLUGIN_VERSION = /\[workspace\.package\][^[]*?\nversion = "([^"]+)"/.exec(
  fs.readFileSync(path.join(REPO, 'Cargo.toml'), 'utf8'),
)?.[1];
/** The built dashboard's build ID (<meta name="atm-dashboard-build">; `pnpm run build` in dashboard/ writes it) */
export const PLUGIN_BUILD = /<meta name="atm-dashboard-build" content="([^"]*)"/.exec(
  fs.readFileSync(path.join(REPO, 'dashboard/dist/tasks-dashboard.html'), 'utf8'),
)?.[1];

/** atm's user folder (ATM_HOME: the project registry, the server record) for tests that don't pick their own, so no
 * test reads or changes the real ~/.agentic-task-manager. */
const TEST_HOME = tempDir('atm-home-');
process.on('exit', () => fs.rmSync(TEST_HOME, { recursive: true, force: true }));

/** The environment without this machine's agent identity, so every test decides who the agent is. */
export function baseEnv() {
  const env = { ...process.env, ATM_HOME: TEST_HOME };
  delete env.CLAUDE_CODE_SESSION_ID;
  delete env.ATM_AGENT;
  delete env.ATM_NOW;
  return env;
}

/** Run `atm args...`; returns { code, stdout, stderr }. */
export function runAtm(args, cwd, env = {}) {
  const result = spawnSync(ATM, args, {
    cwd,
    encoding: 'utf8',
    env: { ...baseEnv(), ...env },
  });
  if (result.error) throw result.error;
  return { code: result.status, stdout: result.stdout, stderr: result.stderr };
}

export function git(args, cwd) {
  const result = spawnSync('git', args, { cwd, encoding: 'utf8' });
  if (result.status !== 0) throw new Error(`git ${args.join(' ')} failed: ${result.stderr}`);
  return result.stdout.trim();
}

/** A fresh temporary directory (real path, so it matches what the scripts report on macOS). */
export function tempDir(prefix = 'atm-test-') {
  return fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), prefix)));
}

/**
 * Create a project from a scenario `setup`:
 *   files:  { "relative/path": "text" | <JSON value> }   written before anything else
 *   git:    true to `git init`; commit: true to also commit everything
 *   commands: [["git", "mv", ...], ...] extra commands run after the commit, with $ATM set to the launcher
 */
export function makeProject(setup = {}) {
  const root = tempDir();
  for (const [file, content] of Object.entries(setup.files || {})) {
    const full = path.join(root, file);
    fs.mkdirSync(path.dirname(full), { recursive: true });
    fs.writeFileSync(full, typeof content === 'string' ? content : JSON.stringify(content, null, 2) + '\n');
  }
  if (setup.git || setup.commit) {
    git(['init', '-q', '-b', 'main'], root);
    git(['config', 'user.email', 'test@example.com'], root);
    git(['config', 'user.name', 'atm test'], root);
    git(['config', 'commit.gpgsign', 'false'], root);
  }
  if (setup.commit) {
    git(['add', '-A'], root);
    git(['commit', '-q', '-m', 'setup', '--allow-empty'], root);
  }
  for (const [cmd, ...args] of setup.commands || []) {
    const env = { ...baseEnv(), ATM };
    const result = spawnSync(cmd, args, { cwd: root, encoding: 'utf8', env });
    if (result.status !== 0) throw new Error(`${cmd} ${args.join(' ')} failed: ${result.stderr}`);
  }
  return root;
}

/** Replace {root}, {version} and {build} placeholders in strings anywhere inside an expected value. */
export function fill(value, vars) {
  if (typeof value === 'string') return value.replace(/\{(root|version|build)\}/g, (_, k) => vars[k]);
  if (Array.isArray(value)) return value.map((v) => fill(v, vars));
  if (value && typeof value === 'object')
    return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, fill(v, vars)]));
  return value;
}

export function removeDir(dir) {
  fs.rmSync(dir, { recursive: true, force: true });
}
