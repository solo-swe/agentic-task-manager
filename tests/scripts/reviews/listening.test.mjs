// Agents listening to the reviews of their work (ATM-567): `atm review wait`, the one `atm review watch`, its
// catching up (ATM-624), and atm telling an agent that isn’t watching (ATM-642).
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { describe, test } from 'node:test';
import { git, makeProject, removeDir } from '../helpers.mjs';
import {
  AGENT,
  HAKAN,
  atm,
  background,
  commitFile,
  ok,
  project,
  projectWithSubmodule,
  roundOf,
  task,
  until,
} from './fixtures.mjs';

describe('agents listening (ATM-567)', () => {
  test('wait ends when the reviewer decides, with the comments added meanwhile; at the time up, exit code 3', async () => {
    const root = project();
    try {
      fs.writeFileSync(path.join(root, 'a.txt'), 'changed\n');
      ok(['review', 'create', '--title', 'Wait for it', '--task', 'T-1', '--uncommitted'], root, AGENT);
      const timedOut = background(['review', 'wait', 'CR-1', '--timeout', '1s', '--json'], root, AGENT);
      assert.equal(await timedOut.ended, 3);

      const waiting = background(['review', 'wait', 'CR-1', '--for', 'decision', '--json'], root, AGENT);
      await new Promise((resolve) => setTimeout(resolve, 500));
      ok(['review', 'comment', 'CR-1', '--file', 'a.txt', '--lines', '1', '--body', 'Why changed?'], root, HAKAN);
      ok(['review', 'submit', 'CR-1', '--request-changes'], root, HAKAN);
      assert.equal(await waiting.ended, 0, waiting.stderr());
      const waited = JSON.parse(waiting.lines.join('\n'));
      assert.equal(waited.review.status, 'changes_requested');
      assert.deepEqual(
        waited.comments.map((c) => [c.comment, c.author, c.body]),
        [['c1', 'Hakan Serce', 'Why changed?']],
      );
    } finally {
      removeDir(root);
    }
  });

  test('the agent’s one watch hears the comments about its work only, with their task or none', async () => {
    // T-1 is claude:A's (a.txt), T-2 claude:B's (b.txt); one review of both
    const root = makeProject({
      commit: true,
      files: {
        '.agentic-task-manager/tasks.json': {
          meta: { project: 'Watch' },
          tasks: [
            {
              id: 'T-1',
              name: 'One',
              type: 'feature',
              status: 'in_progress',
              started_by: 'claude:A',
              files: ['a.txt'],
            },
            {
              id: 'T-2',
              name: 'Two',
              type: 'feature',
              status: 'in_progress',
              started_by: 'claude:B',
              files: ['b.txt'],
            },
          ],
        },
        'a.txt': 'a\n',
        'b.txt': 'b\n',
      },
    });
    const A = { ATM_AGENT: 'claude:A' };
    const B = { ATM_AGENT: 'claude:B' };
    let watching;
    try {
      fs.writeFileSync(path.join(root, 'a.txt'), 'a, changed\n');
      fs.writeFileSync(path.join(root, 'b.txt'), 'b, changed\n');
      ok(['review', 'create', '--title', 'Both', '--task', 'T-1', '--task', 'T-2', '--uncommitted'], root, A);

      watching = background(['review', 'watch', '--json', '--timeout', '60s'], root, A);
      await until(() => /Watching the reviews for claude:A/.test(watching.stderr()), 'the watch to start');

      // On B's file: not A's; on A's: T-1; naming T-2 at the whole review: not A's; A's own and B's: never
      ok(['review', 'comment', 'CR-1', '--file', 'b.txt', '--lines', '1', '--body', 'For B.'], root, HAKAN);
      ok(['review', 'comment', 'CR-1', '--file', 'a.txt', '--lines', '1', '--body', 'For A.'], root, HAKAN);
      ok(['review', 'comment', 'CR-1', '--body', 'T-2: rename the flag.'], root, HAKAN);
      ok(['review', 'comment', 'CR-1', '--file', 'a.txt', '--lines', '1', '--body', 'A notes.'], root, A);
      ok(['review', 'comment', 'CR-1', '--file', 'a.txt', '--lines', '1', '--body', 'B on A.'], root, B);
      // A live review on the working copy, a file no task of A's touched: no task
      fs.writeFileSync(path.join(root, 'c.txt'), 'new\n');
      ok(['review', 'live'], root, B);
      ok(['review', 'comment', 'CR-2', '--file', 'c.txt', '--lines', '1', '--body', 'Live, no task.'], root, HAKAN);

      await until(() => watching.lines.length >= 2, 'two comments heard');
      // A decision on a review of A's work is heard too, once (ATM-600)
      ok(['review', 'submit', 'CR-1', '--request-changes'], root, HAKAN);
      await until(() => watching.lines.length >= 3, 'the decision heard');
      await new Promise((resolve) => setTimeout(resolve, 1500)); // nothing else comes
      // (in the order written within what is heard at once; a slow machine may hear them in two batches)
      const heard = watching.lines
        .map((l) => JSON.parse(l))
        .map((h) => [h.review, h.task, h.body ?? h.decision])
        .sort();
      assert.deepEqual(heard, [
        ['CR-1', 'T-1', 'For A.'],
        ['CR-1', 'T-1', 'changes_requested'],
        ['CR-2', null, 'Live, no task.'],
      ]);
    } finally {
      watching?.stop();
      removeDir(root);
    }
  });

  // ATM-624 (Hakan): approvals waited unseen because the agent's watch had lapsed (a harness ends it); a new watch
  // tells what came meanwhile, once
  test('a watch catches up on what came while none was watching, once; --once stops after it; --since', () => {
    // T-1 is the agent's (a.txt): what is said about it is the agent's to hear
    const root = makeProject({
      commit: true,
      files: {
        '.agentic-task-manager/tasks.json': {
          meta: { project: 'Catch up' },
          tasks: [
            {
              id: 'T-1',
              name: 'One',
              type: 'feature',
              status: 'in_progress',
              started_by: AGENT.ATM_AGENT,
              files: ['a.txt'],
            },
          ],
        },
        'a.txt': 'a\n',
      },
    });
    const OTHER = { ATM_AGENT: 'claude:other' };
    try {
      fs.writeFileSync(path.join(root, 'a.txt'), 'changed\n');
      ok(['review', 'create', '--title', 'Catch up', '--task', 'T-1', '--uncommitted'], root, AGENT);
      // The first watch ever starts from now: nothing to tell, so --once waits out its time, exit code 3
      assert.equal(atm(['review', 'watch', '--once', '--timeout', '1s'], root, AGENT).code, 3);
      assert.equal(atm(['review', 'watch', '--once', '--timeout', '1s'], root, OTHER).code, 3);
      const before = new Date(Date.now() - 1000).toISOString().replace(/\.\d+Z$/, 'Z');
      // While no watch runs: a comment and a decision
      ok(['review', 'comment', 'CR-1', '--file', 'a.txt', '--lines', '1', '--body', 'Why changed?'], root, HAKAN);
      ok(['review', 'submit', 'CR-1', '--request-changes'], root, HAKAN);
      const told = (r) =>
        r.stdout
          .trim()
          .split('\n')
          .filter(Boolean)
          .map((l) => JSON.parse(l));
      // The next watch tells both at once, in order, and --once stops there
      const caught = atm(['review', 'watch', '--once', '--json', '--timeout', '10s'], root, AGENT);
      assert.equal(caught.code, 0, caught.stderr);
      assert.deepEqual(
        told(caught).map((h) => h.body ?? h.decision),
        ['Why changed?', 'changes_requested'],
      );
      // Told once: the watch after it has nothing old
      assert.equal(atm(['review', 'watch', '--once', '--timeout', '1s'], root, AGENT).code, 3);
      // Each agent has its own cursor: the other agent's watch (not its work) tells nothing
      assert.equal(atm(['review', 'watch', '--once', '--timeout', '1s'], root, OTHER).code, 3);
      // --since tells from that time, whatever was told
      const since = atm(['review', 'watch', '--once', '--json', '--since', before, '--timeout', '10s'], root, AGENT);
      assert.equal(since.code, 0, since.stderr);
      assert.deepEqual(
        told(since).map((h) => h.body ?? h.decision),
        ['Why changed?', 'changes_requested'],
      );
      assert.match(
        atm(['review', 'watch', '--since', 'yesterday', '--timeout', '1s'], root, AGENT).stderr,
        /--since needs a time like/,
      );
    } finally {
      removeDir(root);
    }
  });

  // ATM-642 (Hakan): an agent's watch stopped and comments, a poke and approvals went unseen. atm knows whether a
  // watch runs, says so in `loop next --json`, and reminds the agent on stderr while a review of its work is open
  test('an agent not watching its open reviews is told so: on stderr, once a while; loop next says it; never a person', async () => {
    const root = makeProject({
      commit: true,
      files: {
        '.agentic-task-manager/tasks.json': {
          meta: { project: 'Watch' },
          tasks: [
            {
              id: 'T-1',
              name: 'One',
              type: 'feature',
              status: 'in_progress',
              started_by: AGENT.ATM_AGENT,
              files: ['a.txt'],
            },
            { id: 'T-2', name: 'Two', type: 'feature', status: 'todo' },
          ],
        },
        'a.txt': 'a\n',
      },
    });
    const reminded = (r) => /you aren't watching your reviews/.test(r.stderr);
    let watching;
    try {
      // Nothing of its open: nothing said
      assert.equal(reminded(ok(['task', 'list'], root, AGENT)), false);
      fs.writeFileSync(path.join(root, 'a.txt'), 'changed\n');
      // Its review open (by the command itself), no watch: said at once, after the command's own output, on stderr
      // only, naming it
      const first = ok(['review', 'create', '--title', 'Watch me', '--task', 'T-1', '--uncommitted'], root, AGENT);
      assert.ok(reminded(first), first.stderr);
      assert.match(first.stderr, /1 open: CR-1/);
      assert.doesNotMatch(first.stdout, /watching/);
      // Said a moment ago, nothing new: quiet
      assert.equal(reminded(ok(['task', 'list'], root, AGENT)), false);
      // A person is never told; nor a command run as someone else
      assert.equal(reminded(ok(['task', 'list'], root, HAKAN)), false);
      // loop next --json says it, its JSON clean
      const next = JSON.parse(ok(['loop', 'next', '--json'], root, AGENT).stdout);
      assert.deepEqual(next.watch, { running: false, open: ['CR-1'], since_last_watch: [] });
      // A watch running: loop next says so, and nothing is said
      watching = background(['review', 'watch', '--json', '--timeout', '60s'], root, AGENT);
      await until(() => /Watching the reviews/.test(watching.stderr()), 'the watch to start');
      const live = JSON.parse(ok(['loop', 'next', '--json'], root, AGENT).stdout);
      assert.deepEqual(live.watch, { running: true, open: [], since_last_watch: [] });
    } finally {
      watching?.stop();
      removeDir(root);
    }
  });

  // ATM-663 (R5): the watch marked a comment seen before judging it, so one not about the agent's work then was never
  // told, even once it was
  describe('what isn’t about the agent’s work yet is judged again, and told once it is (ATM-663)', () => {
    const A = { ATM_AGENT: 'claude:A' };
    const B = { ATM_AGENT: 'claude:B' };
    const told = (r) =>
      r.stdout
        .trim()
        .split('\n')
        .filter(Boolean)
        .map((l) => JSON.parse(l));
    const watchOnce = (root, env, timeout = '1s') =>
      atm(['review', 'watch', '--once', '--json', '--timeout', timeout], root, env);

    test('a comment on a file the agent touches only in a later commit is told after that commit', () => {
      // T-1 is A's, with no file yet; T-2 B's (b.txt); a review of both, of B's commit to b.txt and c.txt
      const root = makeProject({
        commit: true,
        files: {
          '.agentic-task-manager/tasks.json': {
            meta: { project: 'Later' },
            tasks: [
              { id: 'T-1', name: 'One', type: 'feature', status: 'in_progress', started_by: 'claude:A' },
              {
                id: 'T-2',
                name: 'Two',
                type: 'feature',
                status: 'in_progress',
                started_by: 'claude:B',
                files: ['b.txt'],
              },
            ],
          },
          'a.txt': 'a\n',
          'b.txt': 'b\n',
          'c.txt': 'c\n',
        },
      });
      try {
        fs.writeFileSync(path.join(root, 'b.txt'), 'b, changed\n');
        commitFile(root, 'c.txt', 'c, changed\n', 'b and c');
        ok(
          ['review', 'create', '--title', 'Both', '--task', 'T-1', '--task', 'T-2', '--range', 'HEAD~1..HEAD'],
          root,
          B,
        );
        // A's first watch starts from now
        assert.equal(watchOnce(root, A).code, 3);
        ok(['review', 'comment', 'CR-1', '--file', 'c.txt', '--lines', '1', '--body', 'Who owns c?'], root, HAKAN);
        // Not A's yet: no task of A's touches c.txt
        assert.equal(watchOnce(root, A).code, 3);
        // A's commit touches c.txt: the comment is A's now, about T-1, told once
        fs.writeFileSync(path.join(root, 'c.txt'), 'c, A’s now\n');
        ok(['vcs', 'commit', 'T-1', '--file', 'c.txt'], root, A);
        const after = watchOnce(root, A, '10s');
        assert.equal(after.code, 0, after.stderr);
        assert.deepEqual(
          told(after).map((h) => [h.review, h.task, h.body]),
          [['CR-1', 'T-1', 'Who owns c?']],
        );
        assert.equal(watchOnce(root, A).code, 3, 'told once');
      } finally {
        removeDir(root);
      }
    });

    test('a comment and a decision made before the agent starts the task are told once it starts', () => {
      const root = makeProject({
        commit: true,
        files: {
          '.agentic-task-manager/tasks.json': {
            meta: { project: 'Before' },
            tasks: [{ id: 'T-1', name: 'One', type: 'feature', status: 'todo', files: ['a.txt'] }],
          },
          'a.txt': 'a\n',
        },
      });
      try {
        // B worked T-1 and asked for its review; A's watch starts, and nothing is A's
        ok(['task', 'state', 'T-1', 'in_progress'], root, B);
        fs.writeFileSync(path.join(root, 'a.txt'), 'a, changed\n');
        ok(['review', 'create', '--title', 'One', '--task', 'T-1', '--uncommitted'], root, B);
        assert.equal(watchOnce(root, A).code, 3);
        ok(['review', 'comment', 'CR-1', '--file', 'a.txt', '--lines', '1', '--body', 'Name it better.'], root, HAKAN);
        ok(['review', 'submit', 'CR-1', '--request-changes'], root, HAKAN);
        assert.equal(watchOnce(root, A).code, 3, 'not A’s work');
        // A takes the task back for the rework: both are A's to hear now
        ok(['task', 'state', 'T-1', 'in_progress'], root, A);
        const after = watchOnce(root, A, '10s');
        assert.equal(after.code, 0, after.stderr);
        assert.deepEqual(
          told(after).map((h) => [h.task, h.body ?? h.decision]),
          [
            ['T-1', 'Name it better.'],
            ['T-1', 'changes_requested'],
          ],
        );
        assert.equal(watchOnce(root, A).code, 3, 'told once');
      } finally {
        removeDir(root);
      }
    });

    // R12: watches of one agent shared one lease, and the first to end removed it while the others ran
    test('two watches of one agent: one ending leaves the other’s lease', async () => {
      const root = project();
      let long;
      try {
        long = background(['review', 'watch', '--json', '--timeout', '6s'], root, AGENT);
        await until(() => /Watching the reviews/.test(long.stderr()), 'the long watch to start');
        const short = background(['review', 'watch', '--json', '--timeout', '1s'], root, AGENT);
        await until(() => /Watching the reviews/.test(short.stderr()), 'the short watch to start');
        await short.ended;
        // At once, before the long one's next look could renew a lease the short one removed (`loop next` may have
        // nothing to hand out: its watch state is said either way)
        const next = JSON.parse(atm(['loop', 'next', '--json'], root, AGENT).stdout);
        assert.equal(next.watch.running, true);
        // The long one ends at its time (a watch killed leaves its lease until it runs out): none runs
        await long.ended;
        const after = JSON.parse(atm(['loop', 'next', '--json'], root, AGENT).stdout);
        assert.equal(after.watch.running, false, 'none runs');
      } finally {
        long?.stop();
        removeDir(root);
      }
    });
  });
});
