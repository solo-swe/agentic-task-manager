// What the code review tests share (they were one file, reviews.test.mjs, until ATM-642's round): the agent and
// the person they act as, a project to review, atm run as either, the submodule fixture, a command in the background.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { ATM, baseEnv, git, makeProject, runAtm } from '../helpers.mjs';

export const AGENT = { ATM_AGENT: 'claude:0f56a916' };
export const HAKAN = { ATM_AGENT: 'Hakan Serce' };

/** A project in git: T-1 and T-2 in progress, reviewed before they are done (`blocker`), `a.txt` committed. */
export function project() {
  return makeProject({
    commit: true,
    files: {
      '.agentic-task-manager/tasks.json': {
        meta: { project: 'Reviews', review: 'blocker' },
        tasks: [
          { id: 'T-1', name: 'One', type: 'feature', status: 'in_progress' },
          { id: 'T-2', name: 'Two', type: 'feature', status: 'in_progress' },
        ],
      },
      'a.txt': 'one\ntwo\nthree\n',
    },
  });
}

export function atm(args, root, env) {
  const r = runAtm(args, root, env);
  return { ...r, json: () => JSON.parse(r.stdout) };
}

export function ok(args, root, env) {
  const r = atm(args, root, env);
  assert.equal(r.code, 0, `atm ${args.join(' ')}: ${r.stderr}`);
  return r;
}

export function task(root, id) {
  return ok(['task', 'details', id, '--json'], root, HAKAN).json()[0];
}

export function commitFile(root, file, text, message) {
  fs.writeFileSync(path.join(root, file), text);
  git(['add', file], root);
  git(['commit', '-q', '-m', message], root);
}

/** `project()` with the submodule `lib` (a repository of its own, `b.txt` committed), committed; and that repository. */
export function projectWithSubmodule() {
  const root = project();
  const sub = makeProject({ commit: true, files: { 'b.txt': 'one\n' } });
  git(['-c', 'protocol.file.allow=always', 'submodule', 'add', '-q', sub, 'lib'], root);
  git(['commit', '-q', '-m', 'add lib'], root);
  return { root, sub };
}

/** The files of a review's round, as `atm review show --json` lists them, and the round's submodule. */
export function roundOf(root, review, number) {
  const shown = ok(['review', 'show', review, '--json'], root, HAKAN).json();
  const round = shown.rounds.find((r) => r.number === number);
  return {
    submodule: round.submodule ?? null,
    files: round.files.map((f) => f.path),
  };
}

/** Start atm in the background; its stdout's lines as they come, its end, and a way to stop it. */
export function background(args, root, env) {
  const child = spawn(ATM, args, { cwd: root, env: { ...baseEnv(), ...env } });
  const lines = [];
  let stdout = '';
  let stderr = '';
  child.stdout.on('data', (d) => {
    stdout += d;
    const parts = stdout.split('\n');
    stdout = parts.pop();
    lines.push(...parts.filter(Boolean));
  });
  child.stderr.on('data', (d) => (stderr += d));
  const ended = new Promise((resolve) => child.on('close', (code) => resolve(code)));
  return { lines, ended, stderr: () => stderr, stop: () => child.kill() };
}

/** Until `check()` holds, or fail after `ms`. */
export async function until(check, what, ms = 20_000) {
  const end = Date.now() + ms;
  while (!check()) {
    if (Date.now() > end) throw new Error(`timed out waiting for ${what}`);
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
}
