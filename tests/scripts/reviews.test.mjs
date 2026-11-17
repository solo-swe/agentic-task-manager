// `atm review` (docs/code-review-design.md §3.9, ATM-566): code reviews recorded in the project, through the CLI as
// agents and people run it, with the tasks they review kept in step (ATM-565).
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { describe, test } from 'node:test';
import { git, makeProject, removeDir, runAtm } from './helpers.mjs';

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
