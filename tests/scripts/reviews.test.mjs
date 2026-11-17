// `atm review` (docs/code-review-design.md §3.9, ATM-566): code reviews recorded in the project, through the CLI as
// agents and people run it, with the tasks they review kept in step (ATM-565).
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { describe, test } from 'node:test';
import { ATM, baseEnv, git, makeProject, removeDir, runAtm } from './helpers.mjs';

const AGENT = { ATM_AGENT: 'claude:0f56a916' };
const HAKAN = { ATM_AGENT: 'Hakan Serce' };

/** A project in git: T-1 and T-2 in progress, reviewed before they are done (`blocker`), `a.txt` committed. */
function project() {
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

function atm(args, root, env) {
  const r = runAtm(args, root, env);
  return { ...r, json: () => JSON.parse(r.stdout) };
}

function ok(args, root, env) {
  const r = atm(args, root, env);
  assert.equal(r.code, 0, `atm ${args.join(' ')}: ${r.stderr}`);
  return r;
}

function task(root, id) {
  return ok(['task', 'details', id, '--json'], root, HAKAN).json()[0];
}

function commitFile(root, file, text, message) {
  fs.writeFileSync(path.join(root, file), text);
  git(['add', file], root);
  git(['commit', '-q', '-m', message], root);
}

describe('atm review', () => {
  test('a task through a review: commented, sent back, a round, approved once its comments are resolved', () => {
    const root = project();
    try {
      commitFile(root, 'a.txt', 'one\nTWO\nthree\n', 'the change');
      const opened = ok(
        ['review', 'create', '--title', 'Two in capitals', '--task', 'T-1', '--range', 'HEAD~1..HEAD', '--json'],
        root,
        AGENT,
      ).json();
      assert.equal(opened.reference, 'CR-1');
      assert.equal(opened.status, 'open');
      assert.equal(task(root, 'T-1').status, 'in_review');
      assert.equal(task(root, 'T-1').code_reviews[0].ref, 'CR-1');

      // The reviewer comments on the file and sends it back: the task has changes asked for, naming the review
      const comment = ok(
        [
          'review',
          'comment',
          'CR-1',
          '--file',
          'a.txt',
          '--lines',
          '2',
          '--quote',
          'TWO',
          '--body',
          'Lower case.',
          '--json',
        ],
        root,
        HAKAN,
      ).json();
      assert.equal(comment.comment, 'c1');
      ok(['review', 'submit', 'CR-1', '--request-changes'], root, HAKAN);
      assert.equal(task(root, 'T-1').review_state, 'changes_requested');
      assert.match(JSON.stringify(task(root, 'T-1').notes), /CR-1 round 1: 1 open comment/);

      // The agent replies, takes the task back, fixes it and adds a round: in review again
      ok(['review', 'reply', 'CR-1', 'c1', '--body', 'Done in round 2.'], root, AGENT);
      ok(['task', 'state', 'T-1', 'in_progress'], root, AGENT);
      commitFile(root, 'a.txt', 'one\ntwo, again\nthree\n', 'the fix');
      const round = ok(['review', 'round', 'CR-1', '--range', 'HEAD~2..HEAD', '--json'], root, AGENT).json();
      assert.equal(round.round, 2);
      assert.equal(task(root, 'T-1').status, 'in_review');
      // Round 2 against round 1: only the fix
      const between = ok(['review', 'show', 'CR-1', '--round', '2', '--against', '1'], root, HAKAN).stdout;
      assert.match(between, /-TWO\n\+two, again/);

      // Approving names what is still open; the agent may not resolve the reviewer's comment, nor decide
      const refused = atm(['review', 'submit', 'CR-1', '--approve'], root, HAKAN);
      assert.equal(refused.code, 1);
      assert.match(refused.stderr, /comments are open: c1/);
      assert.notEqual(atm(['review', 'resolve', 'CR-1', 'c1'], root, AGENT).code, 0);
      assert.notEqual(atm(['review', 'submit', 'CR-1', '--approve'], root, AGENT).code, 0);
      ok(['review', 'resolve', 'CR-1', 'c1'], root, HAKAN);
      ok(['review', 'submit', 'CR-1', '--approve'], root, HAKAN);
      assert.equal(task(root, 'T-1').review_state, 'approved');
      ok(['task', 'state', 'T-1', 'done'], root, AGENT);

      // The comments, as comments.md has them, and the files in the project
      const comments = ok(['review', 'comments', 'CR-1'], root, HAKAN).stdout;
      assert.match(
        comments,
        /^## a\.txt:2 · round 1 · t1\n> TWO\n\n- \[x\] \*\*Hakan Serce\*\* · .* · c1\n  Lower case\./,
      );
      assert.match(comments, /\n  - \*\*claude:0f56a916\*\* · .* · c2\n    Done in round 2\./);
      const folders = fs.readdirSync(path.join(root, '.agentic-task-manager/reviews'));
      assert.ok(
        folders.some((f) => /^CR-1-[0-9a-f]{4}$/.test(f)),
        folders.join(', '),
      );
      // The task folder is never part of a round
      const diff = ok(['review', 'show', 'CR-1', '--round', '1'], root, HAKAN).stdout;
      assert.doesNotMatch(diff, /tasks\.json/);
    } finally {
      removeDir(root);
    }
  });

  test('one review of two tasks: both in review, the decision both tasks’, tasks named later', () => {
    const root = project();
    try {
      fs.writeFileSync(path.join(root, 'a.txt'), 'changed\n');
      ok(['review', 'create', '--title', 'Both', '--task', 'T-1', '--uncommitted'], root, AGENT);
      assert.equal(task(root, 'T-1').status, 'in_review');
      assert.equal(task(root, 'T-2').status, 'in_progress');
      ok(['review', 'tasks', 'CR-1', '--add', 'T-2'], root, AGENT);
      assert.equal(task(root, 'T-2').status, 'in_review');
      ok(['review', 'submit', 'CR-1', '--approve', '--body', 'Good.'], root, HAKAN);
      assert.equal(task(root, 'T-1').review_state, 'approved');
      assert.equal(task(root, 'T-2').review_state, 'approved');
      const listed = ok(['review', 'list', '--json'], root, HAKAN).json();
      assert.deepEqual(listed[0].review.tasks, ['T-1', 'T-2']);
      assert.equal(listed[0].branch, 'main');
      assert.equal(ok(['review', 'list', '--search', 'good both'], root, HAKAN).stdout.trim().split('\n').length, 1);
      assert.match(ok(['review', 'list', '--status', 'open'], root, HAKAN).stdout, /No reviews/);
    } finally {
      removeDir(root);
    }
  });

  test('an approved live review covers its task’s done until its change moves', () => {
    const root = project();
    try {
      fs.writeFileSync(path.join(root, 'a.txt'), 'one\ntwo\nthree\nfour\n');
      ok(['review', 'live', '--task', 'T-1'], root, AGENT);
      assert.equal(task(root, 'T-1').status, 'in_progress');
      ok(['review', 'submit', 'CR-1', '--approve'], root, HAKAN);
      // Changed since the approval: the agent may not be done without another review
      fs.writeFileSync(path.join(root, 'a.txt'), 'one\ntwo\nthree\nfour\nfive\n');
      assert.notEqual(atm(['task', 'state', 'T-1', 'done'], root, AGENT).code, 0);
      // As approved: done, the approval naming the live review
      fs.writeFileSync(path.join(root, 'a.txt'), 'one\ntwo\nthree\nfour\n');
      const done = ok(['task', 'state', 'T-1', 'done'], root, AGENT);
      assert.match(done.stdout, /approved in the live review CR-1/);
      assert.equal(task(root, 'T-1').status, 'done');
    } finally {
      removeDir(root);
    }
  });

  test('a live review a person opens is theirs to decide; the change is the agent’s, whose first round names it', () => {
    const root = project();
    try {
      fs.writeFileSync(path.join(root, 'a.txt'), 'one\ntwo\nthree\nfour\n');
      // Hakan opens it on the working copy, no agent at work there and no task named: nobody's change yet
      let header = ok(['review', 'live', '--json'], root, HAKAN).json();
      assert.deepEqual([header.author, header.reviewers], [null, ['Hakan Serce']]);
      assert.match(ok(['review', 'show', 'CR-1'], root, HAKAN).stdout, /by nobody yet/);
      ok(['review', 'comment', 'CR-1', '--file', 'a.txt', '--lines', '1', '--body', 'Why one?'], root, HAKAN);
      // His own round would make him the change's author: refused. The agent ends it with its round: its author now
      assert.match(atm(['review', 'round', 'CR-1'], root, HAKAN).stderr, /not a reviewer's/);
      ok(['review', 'round', 'CR-1'], root, AGENT);
      header = ok(['review', 'show', 'CR-1', '--json'], root, HAKAN).json().header;
      assert.deepEqual([header.author, header.status], [AGENT.ATM_AGENT, 'open']);
      // Hakan decides it, as its reviewer, once his comment is resolved; the agent can't
      ok(['review', 'resolve', 'CR-1', 'c1'], root, HAKAN);
      assert.notEqual(atm(['review', 'submit', 'CR-1', '--approve'], root, AGENT).code, 0);
      ok(['review', 'submit', 'CR-1', '--approve'], root, HAKAN);
    } finally {
      removeDir(root);
    }
  });

  test('a person may override who decides, saying why: recorded in the review', () => {
    const root = project();
    try {
      fs.writeFileSync(path.join(root, 'a.txt'), 'changed\n');
      // Hakan's own review: by the rules not his to decide, unless he overrides that, with a reason
      ok(['review', 'create', '--title', 'Mine', '--uncommitted'], root, HAKAN);
      assert.match(atm(['review', 'submit', 'CR-1', '--approve'], root, HAKAN).stderr, /may override that/);
      assert.notEqual(atm(['review', 'submit', 'CR-1', '--approve', '--override-rules', ' '], root, HAKAN).code, 0);
      ok(['review', 'submit', 'CR-1', '--approve', '--override-rules', 'Ana reviewed it on a call.'], root, HAKAN);
      const review = ok(['review', 'show', 'CR-1', '--json'], root, HAKAN).json();
      assert.equal(review.header.status, 'approved');
      const record = ok(['review', 'comments', 'CR-1', '--json'], root, HAKAN).json().at(-1).comments[0];
      assert.match(record.body, /rules were overridden.*Hakan Serce is its author.*Ana reviewed it on a call\./);
      assert.equal(record.resolved, true);
    } finally {
      removeDir(root);
    }
  });

  test('a comment can’t drive the terminal: its escapes print as text, --json exactly', () => {
    const root = project();
    try {
      fs.writeFileSync(path.join(root, 'a.txt'), 'changed\n');
      ok(['review', 'create', '--title', 'Escapes', '--uncommitted'], root, AGENT);
      const body = 'look\u001b]52;c;aGVsbG8=\u0007 here\u001b[2J';
      ok(['review', 'comment', 'CR-1', '--body', body], root, HAKAN);
      const shown = ok(['review', 'comments', 'CR-1'], root, HAKAN).stdout;
      assert.doesNotMatch(shown, /\u001b|\u0007/);
      assert.match(shown, /look\]52;c;aGVsbG8= here\[2J/);
      const json = ok(['review', 'comments', 'CR-1', '--json'], root, HAKAN).json();
      assert.equal(json[0].comments[0].body, body);
    } finally {
      removeDir(root);
    }
  });

  test('wrong uses are refused naming why', () => {
    const root = project();
    try {
      assert.match(atm(['review', 'create', '--title', 'x'], root, AGENT).stderr, /say what to review/);
      assert.match(atm(['review', 'show', 'CR-9'], root, AGENT).stderr, /no review CR-9/);
      fs.writeFileSync(path.join(root, 'a.txt'), 'changed\n');
      ok(['review', 'create', '--title', 'x', '--uncommitted'], root, AGENT);
      assert.match(atm(['review', 'comment', 'CR-1'], root, HAKAN).stderr, /--body or --body-file/);
      assert.match(
        atm(['review', 'comment', 'CR-1', '--file', 'a.txt', '--lines', '3-1', '--body', 'x'], root, HAKAN).stderr,
        /--lines takes/,
      );
      assert.match(atm(['review', 'list', '--status', 'nope'], root, HAKAN).stderr, /no status nope/);
    } finally {
      removeDir(root);
    }
  });
});

/** Start atm in the background; its stdout's lines as they come, its end, and a way to stop it. */
function background(args, root, env) {
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
async function until(check, what, ms = 20_000) {
  const end = Date.now() + ms;
  while (!check()) {
    if (Date.now() > end) throw new Error(`timed out waiting for ${what}`);
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
}

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
      await new Promise((resolve) => setTimeout(resolve, 1500)); // nothing else comes
      // (in the order written within what is heard at once; a slow machine may hear them in two batches)
      const heard = watching.lines
        .map((l) => JSON.parse(l))
        .map((h) => [h.review, h.task, h.body])
        .sort();
      assert.deepEqual(heard, [
        ['CR-1', 'T-1', 'For A.'],
        ['CR-2', null, 'Live, no task.'],
      ]);
    } finally {
      watching?.stop();
      removeDir(root);
    }
  });
});
