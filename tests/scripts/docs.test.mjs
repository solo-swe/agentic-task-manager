// `atm docs` (T-440): the reference agents read is atm's own. Every topic in docs/agent/ is built into atm.py, and
// every atm command a topic names exists, so the reference can't drift from the command tree.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { describe, test } from 'node:test';
import { REPO, removeDir, runAtm, tempDir } from './helpers.mjs';

const DOCS = path.join(REPO, 'docs/agent');
const topics = fs
  .readdirSync(DOCS)
  .filter((f) => f.endsWith('.md'))
  .map((f) => f.slice(0, -3))
  .sort();

describe('atm docs', () => {
  test('every topic in docs/agent/ is listed, and prints as written', () => {
    const cwd = tempDir();
    try {
      const listed = JSON.parse(runAtm(['docs', '--json'], cwd).stdout).map((t) => t.topic);
      // ... and CompMD's own guide (docs/agent/compmd-llms.txt, T-441)
      assert.deepEqual(listed.sort(), [...topics, 'compmd'].sort());
      const compmd = runAtm(['docs', 'compmd'], cwd);
      assert.equal(compmd.stdout, fs.readFileSync(path.join(DOCS, 'compmd-llms.txt'), 'utf8').replace(/\n*$/, '\n'));
      for (const topic of topics) {
        const r = runAtm(['docs', topic], cwd);
        assert.equal(r.code, 0, `${topic}: ${r.stderr}`);
        assert.equal(r.stdout, fs.readFileSync(path.join(DOCS, `${topic}.md`), 'utf8').replace(/\n*$/, '\n'), topic);
      }
    } finally {
      removeDir(cwd);
    }
  });

  test('every atm command a topic names exists', () => {
    const cwd = tempDir();
    try {
      // `atm <group> <command>` and `atm <command>` in backticks, up to the first argument or option
      const named = new Set();
      for (const topic of topics) {
        const text = fs.readFileSync(path.join(DOCS, `${topic}.md`), 'utf8');
        for (const [, words] of text.matchAll(/`atm ((?:[a-z][a-z-]*)(?: [a-z][a-z-]*)?)/g)) named.add(words);
      }
      assert.ok(named.size > 20, `only ${named.size} commands named`);
      // A group's second word is one of its commands (`atm task details`); a command's is its argument (`atm docs claims`)
      const isGroup = (word) => /\nCommands:/.test(runAtm([word, '--help'], cwd).stdout);
      const exists = (words) => {
        const [first, second] = words.split(' ');
        if (runAtm([first, '--help'], cwd).code !== 0) return false;
        return !second || !isGroup(first) || runAtm([first, second, '--help'], cwd).code === 0;
      };
      assert.ok(!exists('task bogus') && !exists('bogus') && exists('task details') && exists('docs claims'));
      assert.deepEqual(
        [...named].filter((words) => !exists(words)),
        [],
      );
    } finally {
      removeDir(cwd);
    }
  });
});
