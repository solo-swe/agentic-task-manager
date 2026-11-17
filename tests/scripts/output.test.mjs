// atm's styled output (T-488, docs/cli-output-design.md): styles only when asked for or on a terminal, never in a pipe,
// with NO_COLOR, or with --json. The suite runs atm in a pipe, so the scenarios check the plain text; this file checks
// the styles.
import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { makeProject, removeDir, runAtm } from './helpers.mjs';

const ESC = '\u001b[';
const project = () =>
  makeProject({
    files: {
      '.agentic-task-manager/tasks.json': {
        meta: { project: 'Styles' },
        tasks: [
          { id: 'T-1', name: 'Urgent', status: 'blocked', priority: 'P0', labels: ['ux'] },
          { id: 'T-2', name: 'Shipped', status: 'done', priority: 'P2' },
        ],
      },
    },
  });

describe('styles', () => {
  test('a pipe gets plain text, with and without NO_COLOR', () => {
    const root = project();
    try {
      for (const env of [{}, { NO_COLOR: '1' }, { CLICOLOR_FORCE: '' }]) {
        for (const args of [['task', 'list'], ['project', 'status'], ['task', 'details', 'T-1'], ['doctor']]) {
          const out = runAtm(args, root, env);
          // doctor ends with 1 when it reports errors: the output counts here, not the verdict
          assert.ok(out.code <= 1 && out.stdout.length > 0, `${args.join(' ')}: ${out.stderr}`);
          assert.ok(!out.stdout.includes(ESC), `${args.join(' ')} styled in a pipe: ${JSON.stringify(out.stdout)}`);
        }
      }
    } finally {
      removeDir(root);
    }
  });

  test('--color always styles headings, status kinds and priorities; --color never and NO_COLOR with auto do not', () => {
    const root = project();
    try {
      const list = runAtm(['task', 'list', '--color', 'always'], root);
      assert.equal(list.code, 0, list.stderr);
      // blocked is yellow, done green, the first priority red and bold, labels dimmed
      assert.match(list.stdout, /\u001b\[33mblocked\u001b\[0m/);
      assert.match(list.stdout, /\u001b\[32mdone\u001b\[0m/);
      assert.match(list.stdout, /\u001b\[1m\u001b\[31mP0\u001b\[0m|\u001b\[31m\u001b\[1mP0|\u001b\[1;31mP0/);
      assert.match(list.stdout, /\u001b\[2mux\u001b\[0m/);
      const details = runAtm(['task', 'details', 'T-1', '--color', 'always'], root);
      assert.match(details.stdout, /^\u001b\[1mT-1 {2}Urgent\u001b\[0m/);
      // The words are the same as a pipe's
      const plain = runAtm(['task', 'list'], root).stdout;
      assert.equal(list.stdout.replace(/\u001b\[[0-9;]*m/g, ''), plain);

      assert.ok(!runAtm(['task', 'list', '--color', 'never'], root).stdout.includes(ESC));
      assert.ok(!runAtm(['task', 'list', '--color', 'auto'], root, { NO_COLOR: '1' }).stdout.includes(ESC));
    } finally {
      removeDir(root);
    }
  });

  test('task text can’t drive the terminal: its control characters and escape sequences are dropped, atm’s own styles kept', () => {
    // Anyone who can change tasks.json writes this (a merged pull request, another agent): an OSC 52 clipboard write,
    // a screen clear, a window title, a bell, a carriage return over the line, a C1 CSI, and a colour
    const name = 'Hello \u001b]52;c;cHduZWQ=\u0007 \u001b[2J\u001b[8mhidden\u001b[0m x\ry \u009b31m world';
    const root = makeProject({
      files: {
        '.agentic-task-manager/tasks.json': {
          meta: { project: 'Escapes \u001b]0;title\u0007' },
          tasks: [
            {
              id: 'T-1',
              name,
              status: 'todo',
              description: 'Body \u001b]0;title\u0007 text\u0007',
              notes: [{ at: '2026-11-17T00:00:00Z', type: 'note', by: 'x', text: 'note \u001b[1A up' }],
            },
          ],
        },
      },
    });
    try {
      for (const color of ['always', 'never']) {
        for (const args of [
          ['task', 'list'],
          ['task', 'details', 'T-1'],
          ['project', 'status'],
          ['sprint', 'backlog'],
        ]) {
          const out = runAtm([...args, '--color', color], root);
          assert.equal(out.code, 0, `${args.join(' ')}: ${out.stderr}`);
          const what = `${args.join(' ')} --color ${color}: ${JSON.stringify(out.stdout)}`;
          // Nothing but SGR sequences of atm's own styles, and no other control character but the line ends
          const rest = out.stdout.replace(/\u001b\[(?:[0-9]|[34][0-7]|9[0-7]|2[2-4]|39)(?:;[0-9]+)*m/g, '');
          assert.ok(!/[\u0000-\u0009\u000b-\u001f\u007f-\u009f]/.test(rest), what);
          assert.ok(!out.stdout.includes('\u001b[8m'), what);
          assert.ok(out.stdout.includes('Hello') && out.stdout.includes('world'), what);
        }
      }
      // The words stay: only the controls go
      const plain = runAtm(['task', 'list'], root).stdout;
      assert.match(plain, /Hello \]52;c;cHduZWQ= \[2J\[8mhidden\[0m xy 31m world/);
      // --json keeps the text exactly, escaped as JSON escapes it
      assert.equal(JSON.parse(runAtm(['task', 'list', '--json'], root).stdout)[0].name, name);
    } finally {
      removeDir(root);
    }
  });

  test('--json is never styled, even with --color always', () => {
    const root = project();
    try {
      for (const args of [['task', 'list'], ['project', 'status'], ['doctor']]) {
        const out = runAtm([...args, '--json', '--color', 'always'], root);
        assert.ok(out.code <= 1, `${args.join(' ')}: ${out.stderr}`);
        assert.ok(!out.stdout.includes(ESC), args.join(' '));
        JSON.parse(out.stdout);
      }
    } finally {
      removeDir(root);
    }
  });
});
