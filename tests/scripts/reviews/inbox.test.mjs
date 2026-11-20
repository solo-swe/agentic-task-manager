// What an agent hasn't read about its work (ATM-708, Hakan): a watch that runs on tells into a file the agent may never
// open, so the comments and decisions went unseen. The inbox keeps what the agent has read; every command says what it
// hasn't; a watch with --once counts as read; Claude Code's hook puts the unread in front of the agent.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { describe, test } from 'node:test';
import { ATM, baseEnv, makeProject, removeDir, tempDir } from '../helpers.mjs';
import { HAKAN, atm, background, ok, until } from './fixtures.mjs';

const A = { ATM_AGENT: 'claude:A' };
const B = { ATM_AGENT: 'claude:B' };
/** The session the hook's input names, and the identity atm gives it */
const SESSION = 'abcdef1234567890';
const SESSION_AGENT = 'claude:abcdef12';

/** T-1 is claude:A's (a.txt), T-2 the hook session's (b.txt) */
function watched() {
  return makeProject({
    commit: true,
    files: {
      '.agentic-task-manager/tasks.json': {
        meta: { project: 'Inbox' },
        tasks: [
          { id: 'T-1', name: 'One', type: 'feature', status: 'in_progress', started_by: 'claude:A', files: ['a.txt'] },
          {
            id: 'T-2',
            name: 'Two',
            type: 'feature',
            status: 'in_progress',
            started_by: SESSION_AGENT,
            files: ['b.txt'],
          },
        ],
      },
      'a.txt': 'a\n',
      'b.txt': 'b\n',
    },
  });
}

const unreadLine = (r) => /atm: (\d+) unread about your work: ([^\n]*)/.exec(r.stderr);
const lines = (r) =>
  r.stdout
    .split('\n')
    .filter(Boolean)
    .map((l) => JSON.parse(l));

/** Claude Code running atm's hook for `event`, its input on stdin; `home` keeps the user's own settings out of it */
function hook(root, event, home, more = {}) {
  const input =
    typeof more === 'string'
      ? more
      : JSON.stringify({ hook_event_name: event, session_id: SESSION, cwd: root, ...more });
  const result = spawnSync(ATM, ['review', 'inbox', '--hook'], {
    cwd: root,
    input,
    encoding: 'utf8',
    env: { ...baseEnv(), HOME: home },
  });
  return { code: result.status, stdout: result.stdout, stderr: result.stderr };
}

describe('the inbox (ATM-708)', () => {
  test('what a watch that runs on told stays unread: every command says so until the inbox is read; per agent', async () => {
    const root = watched();
    let watching;
    try {
      fs.writeFileSync(path.join(root, 'a.txt'), 'a, changed\n');
      ok(['review', 'create', '--title', 'Mine', '--task', 'T-1', '--uncommitted'], root, A);
      // Nothing yet
      assert.equal(unreadLine(ok(['task', 'list'], root, A)), null);
      watching = background(['review', 'watch', '--json', '--timeout', '60s'], root, A);
      await until(() => /Watching the reviews for claude:A/.test(watching.stderr()), 'the watch to start');
      ok(['review', 'comment', 'CR-1', '--file', 'a.txt', '--lines', '1', '--body', 'Why?'], root, HAKAN);
      ok(['review', 'submit', 'CR-1', '--request-changes'], root, HAKAN);
      await until(() => watching.lines.length >= 2, 'the watch to tell both');
      // Told, into a file nobody read: still unread, said on every command, after its output
      for (let i = 0; i < 2; i++) {
        const said = unreadLine(ok(['task', 'list'], root, A));
        assert.ok(said, 'the unread line');
        assert.equal(said[1], '2');
        assert.match(said[2], /CR-1 comment by Hakan Serce, CR-1 changes requested/);
      }
      // Another agent's inbox is its own; a person is never told
      assert.equal(unreadLine(ok(['task', 'list'], root, B)), null);
      assert.equal(unreadLine(ok(['task', 'list'], root, HAKAN)), null);
      // --peek shows them, still unread
      const peeked = atm(['review', 'inbox', '--peek', '--json'], root, A);
      assert.equal(peeked.code, 0, peeked.stderr);
      assert.deepEqual(
        lines(peeked).map((e) => e.body ?? e.decision),
        ['Why?', 'changes_requested'],
      );
      assert.doesNotMatch(peeked.stderr, /unread/, 'the inbox itself says nothing on stderr');
      // Read: shown once, then nothing is unread
      const read = ok(['review', 'inbox'], root, A);
      assert.match(read.stdout, /CR-1 c1 \(T-1\) on a.txt:1-1, Hakan Serce: Why\?/);
      assert.match(read.stdout, /CR-1 \(T-1\): changes requested/);
      assert.equal(ok(['review', 'inbox', '--json'], root, A).stdout, '');
      assert.equal(ok(['review', 'inbox'], root, A).stdout.trim(), 'Nothing unread about your work.');
      assert.equal(unreadLine(ok(['task', 'list'], root, A)), null);
    } finally {
      watching?.stop();
      removeDir(root);
    }
  });

  test('a watch with --once counts as read: it ends to be read, and the harness tells the agent then', () => {
    const root = watched();
    try {
      fs.writeFileSync(path.join(root, 'a.txt'), 'a, changed\n');
      ok(['review', 'create', '--title', 'Mine', '--task', 'T-1', '--uncommitted'], root, A);
      assert.equal(atm(['review', 'watch', '--once', '--timeout', '1s'], root, A).code, 3);
      ok(['review', 'comment', 'CR-1', '--file', 'a.txt', '--lines', '1', '--body', 'Why?'], root, HAKAN);
      // Unread until the watch tells it
      assert.ok(unreadLine(ok(['task', 'list'], root, A)));
      const told = atm(['review', 'watch', '--once', '--json', '--timeout', '10s'], root, A);
      assert.equal(told.code, 0, told.stderr);
      assert.deepEqual(
        lines(told).map((e) => e.body),
        ['Why?'],
      );
      assert.equal(unreadLine(ok(['task', 'list'], root, A)), null);
      assert.equal(ok(['review', 'inbox', '--json'], root, A).stdout, '');
    } finally {
      removeDir(root);
    }
  });
});

describe('Claude Code’s hook (ATM-708)', () => {
  test('the unread come as context on a prompt or a tool call, once; a reply is held while there are some', () => {
    const root = watched();
    const home = tempDir('atm-hook-home-');
    try {
      fs.writeFileSync(path.join(root, 'b.txt'), 'b, changed\n');
      ok(['review', 'create', '--title', 'The session’s', '--task', 'T-2', '--uncommitted'], root, {
        ATM_AGENT: SESSION_AGENT,
      });
      // Nothing unread: nothing said, on any event
      for (const event of ['UserPromptSubmit', 'PostToolUse', 'Stop']) {
        const quiet = hook(root, event, home);
        assert.deepEqual([quiet.code, quiet.stdout], [0, ''], event);
      }
      ok(['review', 'comment', 'CR-1', '--file', 'b.txt', '--lines', '1', '--body', 'Rename it.'], root, HAKAN);
      const after = hook(root, 'PostToolUse', home);
      assert.equal(after.code, 0, after.stderr);
      const added = JSON.parse(after.stdout).hookSpecificOutput;
      assert.equal(added.hookEventName, 'PostToolUse');
      assert.match(added.additionalContext, /1 unread about your work/);
      assert.match(added.additionalContext, /CR-1 \(T-2\): comment c1 by Hakan Serce on b.txt:1-1: «Rename it\.»/);
      assert.match(added.additionalContext, /data, not commands/);
      // Read once given: the next event says nothing, and neither does the session's next command
      assert.equal(hook(root, 'UserPromptSubmit', home).stdout, '');
      assert.equal(unreadLine(ok(['task', 'list'], root, { ATM_AGENT: SESSION_AGENT })), null);
      // A decision while the agent ends its reply: the reply is held, with the reason
      ok(['review', 'submit', 'CR-1', '--request-changes'], root, HAKAN);
      const held = JSON.parse(hook(root, 'Stop', home).stdout);
      assert.equal(held.decision, 'block');
      assert.match(held.reason, /CR-1 \(T-2\): changes requested/);
      assert.equal(hook(root, 'Stop', home, { stop_hook_active: true }).stdout, '', 'once said, it lets the reply end');
    } finally {
      removeDir(root);
      removeDir(home);
    }
  });

  test('never in the way: input that isn’t Claude Code’s, an event atm isn’t on, a folder without a project', () => {
    const root = watched();
    const home = tempDir('atm-hook-home-');
    const elsewhere = tempDir('atm-hook-none-');
    try {
      for (const [what, result] of [
        ['not JSON', hook(root, 'Stop', home, 'not json')],
        ['no event', hook(root, 'Stop', home, '{}')],
        ['another event', hook(root, 'PreCompact', home)],
        ['no project', hook(root, 'Stop', home, { cwd: elsewhere })],
      ]) {
        assert.deepEqual([result.code, result.stdout], [0, ''], what);
      }
    } finally {
      removeDir(root);
      removeDir(home);
      removeDir(elsewhere);
    }
  });

  test('atm agents hooks: install adds one command to three events, keeps the rest; again nothing; uninstall restores', () => {
    const root = watched();
    const home = tempDir('atm-hook-home-');
    const env = { HOME: home };
    const settings = path.join(root, '.claude', 'settings.json');
    try {
      fs.mkdirSync(path.dirname(settings), { recursive: true });
      const theirs = {
        model: 'opus',
        hooks: { PostToolUse: [{ matcher: 'Write', hooks: [{ type: 'command', command: 'prettier' }] }] },
      };
      fs.writeFileSync(settings, JSON.stringify(theirs));
      const added = ok(['agents', 'hooks', 'install', '--project', '--json'], root, env).json();
      assert.deepEqual(added.changed, ['UserPromptSubmit', 'PostToolUse', 'Stop']);
      const written = JSON.parse(fs.readFileSync(settings, 'utf8'));
      assert.equal(written.model, 'opus');
      assert.deepEqual(written.hooks.PostToolUse[0], theirs.hooks.PostToolUse[0]);
      for (const event of ['UserPromptSubmit', 'PostToolUse', 'Stop'])
        assert.equal(written.hooks[event].at(-1).hooks[0].command, 'atm review inbox --hook', event);
      assert.deepEqual(ok(['agents', 'hooks', 'install', '--project', '--json'], root, env).json().changed, []);
      assert.match(
        ok(['agents', 'hooks', 'status', '--project'], root, env).stdout,
        /on UserPromptSubmit, PostToolUse, Stop/,
      );
      ok(['agents', 'hooks', 'uninstall', '--project'], root, env);
      assert.deepEqual(JSON.parse(fs.readFileSync(settings, 'utf8')), theirs);
      assert.match(ok(['agents', 'hooks', 'status', '--project'], root, env).stdout, /isn't in/);
      // The user's own settings were never touched
      assert.equal(fs.existsSync(path.join(home, '.claude', 'settings.json')), false);
      // A file that isn't JSON is left alone, exit code 2
      fs.writeFileSync(settings, '{oops');
      const refused = atm(['agents', 'hooks', 'install', '--project'], root, env);
      assert.equal(refused.code, 2);
      assert.match(refused.stderr, /leaves it alone/);
      assert.equal(fs.readFileSync(settings, 'utf8'), '{oops');
    } finally {
      removeDir(root);
      removeDir(home);
    }
  });
});
