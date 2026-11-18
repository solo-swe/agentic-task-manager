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

      // Sent back, the review still takes the reviewer's words (ATM-597): a comment, and changes asked for again with
      // a message, which is kept; it stays sent back and the task isn't changed again
      ok(['review', 'comment', 'CR-1', '--body', 'And the title too.'], root, HAKAN);
      ok(['review', 'submit', 'CR-1', '--request-changes', '--body', 'Both, please.'], root, HAKAN);
      const sentBack = ok(['review', 'show', 'CR-1', '--json'], root, HAKAN).json();
      assert.equal(sentBack.header.status, 'changes_requested');
      assert.deepEqual(
        sentBack.chains.map((c) => c.comments[0].body),
        ['Lower case.', 'And the title too.', 'Both, please.'],
      );
      ok(['review', 'resolve', 'CR-1', 'c2'], root, HAKAN);
      ok(['review', 'resolve', 'CR-1', 'c3'], root, HAKAN);
      assert.equal(task(root, 'T-1').review_state, 'changes_requested');

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
      assert.match(comments, /\n  - \*\*claude:0f56a916\*\* · .* · c4\n    Done in round 2\./);
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

  test('each wrong use has its exit code: 2 for a usage error, 1 for what the review or the files refuse', () => {
    const root = project();
    try {
      fs.writeFileSync(path.join(root, 'a.txt'), 'changed\n');
      ok(['review', 'create', '--title', 'x', '--uncommitted'], root, AGENT);
      // [args, exit code, what stderr says]
      const cases = [
        [['review', 'create', '--title', 'x'], 2, /say what to review/],
        [['review', 'create', '--title', 'x', '--range', 'HEAD'], 2, /--range takes BASE\.\.HEAD, not HEAD/],
        [['review', 'create', '--title', 'x', '--uncommitted', '--summary-file', 'gone.md'], 1, /can't read gone\.md/],
        [['review', 'create', '--title', 'x', '--range', 'HEAD~1..HEAD', '--uncommitted'], 2, /cannot be used with/],
        [['review', 'show', 'CR-9'], 1, /no review CR-9/],
        [['review', 'show', 'CR-1', '--against', '1'], 2, /--round/],
        [['review', 'comment', 'CR-1'], 2, /--body or --body-file/],
        [['review', 'comment', 'CR-1', '--body', '   '], 2, /the text is empty/],
        [['review', 'comment', 'CR-1', '--body-file', 'gone.md'], 1, /can't read gone\.md/],
        [['review', 'comment', 'CR-1', '--body', 'x', '--body-file', 'a.txt'], 2, /cannot be used with/],
        [['review', 'comment', 'CR-1', '--lines', '2', '--body', 'x'], 2, /--file/],
        [
          ['review', 'comment', 'CR-1', '--file', 'a.txt', '--lines', '0', '--body', 'x'],
          2,
          /--lines takes A-B or A, not 0/,
        ],
        [['review', 'comment', 'CR-1', '--file', 'a.txt', '--lines', 'two', '--body', 'x'], 2, /--lines takes/],
        [['review', 'comment', 'CR-9', '--body', 'x'], 1, /no review CR-9/],
        [['review', 'reply', 'CR-1', 'c9', '--body', 'x'], 1, /c9/],
        [['review', 'submit', 'CR-1'], 2, /submit with --approve, --request-changes or --comment/],
        [['review', 'submit', 'CR-1', '--comment'], 2, /--comment needs its text/],
        [['review', 'submit', 'CR-1', '--approve', '--request-changes'], 2, /cannot be used with/],
        [['review', 'wait', 'CR-1', '--timeout', '0'], 2, /--timeout takes 90s, 30m, 2h or seconds, not 0/],
        [['review', 'wait', 'CR-1', '--timeout', 'soon'], 2, /--timeout takes/],
        [['review', 'wait', 'CR-1', '--for', 'ever'], 2, /invalid value 'ever'/],
        [['review', 'watch', '--timeout=-5m'], 2, /--timeout takes/],
        [['review', 'close', 'CR-1'], 2, /--reason/],
        [['review', 'list', '--status', 'nope'], 2, /no status nope/],
      ];
      for (const [args, code, stderr] of cases) {
        const r = atm(args, root, HAKAN);
        assert.equal(r.code, code, `atm ${args.join(' ')}: ${r.stderr}`);
        assert.match(r.stderr, stderr, `atm ${args.join(' ')}`);
      }
      // None of them changed the review
      const review = ok(['review', 'show', 'CR-1', '--json'], root, HAKAN).json();
      assert.deepEqual([review.header.status, review.chains.length], ['open', 0]);
    } finally {
      removeDir(root);
    }
  });

  test('--dry-run says what each command would do and writes nothing', () => {
    const root = project();
    try {
      fs.writeFileSync(path.join(root, 'a.txt'), 'changed\n');
      const reviews = path.join(root, '.agentic-task-manager/reviews');
      const create = ok(['review', 'create', '--title', 'Try', '--task', 'T-1', '--uncommitted', '-n'], root, AGENT);
      assert.equal(create.stdout, '[dry-run] would open a review “Try” of 1 file(s)\n');
      assert.equal(
        ok(['review', 'live', '-n'], root, AGENT).stdout,
        '[dry-run] would open a live review “Live review”\n',
      );
      assert.ok(!fs.existsSync(reviews), 'no review written');
      assert.equal(task(root, 'T-1').status, 'in_progress');

      ok(['review', 'create', '--title', 'Real', '--task', 'T-1', '--uncommitted'], root, AGENT);
      ok(['review', 'comment', 'CR-1', '--body', 'Why?'], root, HAKAN);
      const before = ok(['review', 'show', 'CR-1', '--json'], root, HAKAN).stdout;
      const dry = [
        [['review', 'round', 'CR-1', '-n'], AGENT, '[dry-run] would add a round to CR-1\n'],
        [['review', 'tasks', 'CR-1', '--add', 'T-2', '-n'], AGENT, "[dry-run] would change CR-1's tasks\n"],
        [['review', 'comment', 'CR-1', '--body', 'More', '-n'], HAKAN, '[dry-run] would comment on CR-1\n'],
        [['review', 'reply', 'CR-1', 'c1', '--body', 'Because', '-n'], AGENT, '[dry-run] would reply to c1 in CR-1\n'],
        [['review', 'edit', 'CR-1', 'c1', '--body', 'Why not?', '-n'], HAKAN, '[dry-run] would edit c1 in CR-1\n'],
        [['review', 'resolve', 'CR-1', 'c1', '-n'], HAKAN, '[dry-run] would resolve c1 in CR-1\n'],
        [['review', 'resolve', 'CR-1', 'c1', '--reopen', '-n'], HAKAN, '[dry-run] would reopen c1 in CR-1\n'],
        [['review', 'submit', 'CR-1', '--request-changes', '-n'], HAKAN, '[dry-run] would submit on CR-1\n'],
        [['review', 'close', 'CR-1', '--reason', 'No', '-n'], AGENT, '[dry-run] would close CR-1\n'],
      ];
      for (const [args, env, stdout] of dry) assert.equal(ok(args, root, env).stdout, stdout, args.join(' '));
      assert.equal(ok(['review', 'show', 'CR-1', '--json'], root, HAKAN).stdout, before);
      assert.equal(task(root, 'T-2').status, 'in_progress');
    } finally {
      removeDir(root);
    }
  });

  test('a draft of files as they are, with a summary: nothing in review yet; show prints its header and rounds', () => {
    const root = project();
    try {
      fs.writeFileSync(path.join(root, 'summary.md'), 'What it does.\nAnd why.\n');
      const draft = ok(
        [
          'review',
          'create',
          '--title',
          'Early look',
          '--task',
          'T-1',
          '--file',
          'a.txt',
          '--summary-file',
          'summary.md',
          '--reviewer',
          'Ana',
          '--draft',
          '--json',
        ],
        root,
        AGENT,
      ).json();
      assert.deepEqual([draft.status, draft.tasks, draft.reviewers], ['draft', ['T-1'], ['Ana']]);
      assert.equal(task(root, 'T-1').status, 'in_progress');
      const shown = ok(['review', 'show', 'CR-1'], root, HAKAN).stdout;
      assert.match(
        shown,
        /^CR-1 · draft · Early look · T-1\n {2}by claude:0f56a916 · decided by Ana · opened .+\n\n {2}What it does\.\n {2}And why\.\n\n {2}round 1 · files as they are · 1 file\(s\) · claude:0f56a916 · .+\n {2}0 chain\(s\), 0 open\n$/,
      );
      // The next round is files again, unless named
      fs.writeFileSync(path.join(root, 'a.txt'), 'one\ntwo\nthree\nfour\n');
      assert.equal(ok(['review', 'round', 'CR-1'], root, AGENT).stdout, 'CR-1: round 2\n');
      assert.match(ok(['review', 'show', 'CR-1'], root, HAKAN).stdout, /round 2 · files as they are · 1 file\(s\)/);
      // A round with a new summary (its Description): the author's; read before anything is written
      fs.writeFileSync(path.join(root, 'next.md'), '1. Check the fourth line.\n');
      assert.equal(
        ok(['review', 'round', 'CR-1', '--summary-file', 'next.md', '--dry-run'], root, AGENT).stdout,
        '[dry-run] would add a round to CR-1, with a new summary\n',
      );
      ok(['review', 'round', 'CR-1', '--summary-file', 'next.md'], root, AGENT);
      assert.equal(ok(['review', 'show', 'CR-1', '--json'], root, HAKAN).json().summary, '1. Check the fourth line.');
      const missing = atm(['review', 'round', 'CR-1', '--summary-file', 'none.md'], root, AGENT);
      assert.equal(missing.code, 1);
      assert.match(missing.stderr, /can't read none\.md/);
      assert.match(ok(['review', 'show', 'CR-1'], root, HAKAN).stdout, /round 3 ·/);
      assert.doesNotMatch(ok(['review', 'show', 'CR-1'], root, HAKAN).stdout, /round 4 ·/);
    } finally {
      removeDir(root);
    }
  });

  test('edit is the comment author’s own; a reply is resolved through its chain; resolve --reopen opens it again', () => {
    const root = project();
    try {
      fs.writeFileSync(path.join(root, 'a.txt'), 'changed\n');
      ok(['review', 'create', '--title', 'x', '--uncommitted'], root, AGENT);
      assert.equal(ok(['review', 'comments', 'CR-1'], root, HAKAN).stdout, 'CR-1: no comments\n');
      ok(['review', 'comment', 'CR-1', '--body', 'Tpyo.'], root, HAKAN);
      fs.writeFileSync(path.join(root, 'better.md'), 'Typo, line 1.\n');
      assert.equal(
        ok(['review', 'edit', 'CR-1', 'c1', '--body-file', 'better.md'], root, HAKAN).stdout,
        'CR-1: c1 changed\n',
      );
      const edited = atm(['review', 'edit', 'CR-1', 'c1', '--body', 'Mine now'], root, AGENT);
      assert.equal(edited.code, 1);
      assert.match(edited.stderr, /c1 is Hakan Serce's to edit/);
      assert.equal(
        ok(['review', 'comments', 'CR-1', '--json'], root, HAKAN).json()[0].comments[0].body,
        'Typo, line 1.',
      );

      assert.equal(
        ok(['review', 'reply', 'CR-1', 'c1', '--body', 'Fixed.'], root, AGENT).stdout,
        'CR-1: c2, in reply to c1\n',
      );
      const reply = atm(['review', 'resolve', 'CR-1', 'c2'], root, HAKAN);
      assert.equal(reply.code, 2);
      assert.match(reply.stderr, /c2 is a reply: resolve the comment it belongs to \(c1\)/);
      assert.deepEqual(ok(['review', 'resolve', 'CR-1', 'c1', '--json'], root, HAKAN).json(), {
        review: 'CR-1',
        comment: 'c1',
        resolved: true,
      });
      assert.equal(ok(['review', 'comments', 'CR-1', '--open'], root, HAKAN).stdout, 'CR-1: no comments open\n');
      assert.equal(ok(['review', 'resolve', 'CR-1', 'c1', '--reopen'], root, HAKAN).stdout, 'CR-1: c1 open again\n');
      assert.equal(ok(['review', 'comments', 'CR-1', '--open', '--json'], root, HAKAN).json().length, 1);
    } finally {
      removeDir(root);
    }
  });

  test('close ends a review, its author’s or a reviewer’s to do; --reject-tasks rejects its tasks in review; ended, it takes nothing more', () => {
    const root = project();
    try {
      fs.writeFileSync(path.join(root, 'a.txt'), 'changed\n');
      ok(['review', 'create', '--title', 'Kept', '--task', 'T-1', '--uncommitted', '--reviewer', 'Ana'], root, AGENT);
      ok(
        ['review', 'create', '--title', 'Dropped', '--task', 'T-2', '--uncommitted', '--reviewer', 'Ana'],
        root,
        AGENT,
      );
      const stranger = atm(['review', 'close', 'CR-1', '--reason', 'No'], root, HAKAN);
      assert.equal(stranger.code, 1);
      assert.match(stranger.stderr, /CR-1 is closed by its author or a reviewer/);

      assert.equal(
        ok(['review', 'close', 'CR-1', '--reason', 'Split in two'], root, AGENT).stdout,
        'CR-1 · closed · Kept · T-1\n',
      );
      assert.equal(task(root, 'T-1').status, 'in_review');
      const closed = ok(['review', 'close', 'CR-2', '--reason', 'Not needed', '--reject-tasks', '--json'], root, {
        ATM_AGENT: 'Ana',
      }).json();
      assert.equal(closed.status, 'closed');
      const t2 = task(root, 'T-2');
      assert.deepEqual([t2.status, t2.review_state], ['in_review', 'rejected']);
      assert.match(JSON.stringify(t2.notes), /CR-2 closed: Not needed/);
      // The reason is on the record, resolved
      const record = ok(['review', 'comments', 'CR-2', '--json'], root, HAKAN).json()[0].comments[0];
      assert.deepEqual([record.body, record.resolved], ['Not needed', true]);

      // Ended: no comment, reply, round, submit or second close
      for (const args of [
        ['review', 'comment', 'CR-1', '--body', 'Late'],
        ['review', 'reply', 'CR-1', 'c1', '--body', 'Late'],
        ['review', 'submit', 'CR-1', '--approve'],
        ['review', 'close', 'CR-1', '--reason', 'Again'],
        ['review', 'tasks', 'CR-1', '--add', 'T-2'],
      ]) {
        const r = atm(args, root, { ATM_AGENT: 'Ana' });
        assert.equal(r.code, 1, args.join(' '));
        assert.match(r.stderr, /CR-1 is closed: it can't/, args.join(' '));
      }
      assert.equal(ok(['review', 'list', '--status', 'closed'], root, HAKAN).stdout.trim().split('\n').length, 2);
    } finally {
      removeDir(root);
    }
  });

  test('tasks --remove takes a task out; list by --task and --mine', () => {
    const root = project();
    try {
      fs.writeFileSync(path.join(root, 'a.txt'), 'changed\n');
      ok(['review', 'create', '--title', 'Both', '--task', 'T-1', '--task', 'T-2', '--uncommitted'], root, AGENT);
      ok(['review', 'create', '--title', 'Other', '--file', 'a.txt', '--reviewer', 'Ana'], root, HAKAN);
      assert.equal(ok(['review', 'tasks', 'CR-1', '--remove', 'T-2'], root, AGENT).stdout, 'CR-1: T-1\n');
      assert.equal(ok(['review', 'tasks', 'CR-1', '--remove', 'T-1'], root, AGENT).stdout, 'CR-1: no tasks\n');
      ok(['review', 'tasks', 'CR-1', '--add', 'T-2'], root, AGENT);

      const refs = (args, env) =>
        ok(['review', 'list', ...args, '--json'], root, env)
          .json()
          .map((f) => f.review.reference);
      assert.deepEqual(refs(['--task', 'T-2'], HAKAN), ['CR-1']);
      assert.deepEqual(refs(['--task', 'T-1'], HAKAN), []);
      // Mine: the ones I opened or am named to decide (CR-1 names no reviewer)
      assert.deepEqual(refs(['--mine'], AGENT), ['CR-1']);
      assert.deepEqual(refs(['--mine'], { ATM_AGENT: 'Ana' }), ['CR-2']);
      assert.deepEqual(refs(['--mine'], HAKAN), ['CR-2']);
    } finally {
      removeDir(root);
    }
  });
});

/** Start atm in the background; its stdout's lines as they come, its end, and a way to stop it. */
/** `project()` with the submodule `lib` (a repository of its own, `b.txt` committed), committed; and that repository. */
function projectWithSubmodule() {
  const root = project();
  const sub = makeProject({ commit: true, files: { 'b.txt': 'one\n' } });
  git(['-c', 'protocol.file.allow=always', 'submodule', 'add', '-q', sub, 'lib'], root);
  git(['commit', '-q', '-m', 'add lib'], root);
  return { root, sub };
}

/** The files of a review's round, as `atm review show --json` lists them, and the round's submodule. */
function roundOf(root, review, number) {
  const shown = ok(['review', 'show', review, '--json'], root, HAKAN).json();
  const round = shown.rounds.find((r) => r.number === number);
  return { submodule: round.submodule ?? null, files: round.files.map((f) => f.path) };
}

describe('a provisional task key in reviews (ATM-594)', () => {
  const PROVISIONAL = '0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8012';

  /** A project in task keys' format, in git: ATM-12 provisional and in progress (`blocker`), `a.txt` committed. */
  function keyed() {
    return makeProject({
      commit: true,
      files: {
        '.agentic-task-manager/tasks.json': {
          meta: { project: 'Keyed', format: 2, key: 'ATM', next_number: 13, review: 'blocker' },
          tasks: [
            {
              id: PROVISIONAL,
              number: 12,
              key_final: false,
              name: 'Provisional',
              type: 'feature',
              status: 'in_progress',
              started_by: AGENT.ATM_AGENT,
            },
          ],
        },
        'a.txt': 'one\n',
      },
    });
  }

  test('what atm prints names the task by its key; given back, with or without its ~, it works; the approval reaches it', () => {
    const root = keyed();
    try {
      // Named as people read it, with the mark
      commitFile(root, 'a.txt', 'one\ntwo\n', 'the change');
      const created = ok(
        ['review', 'create', '--title', 'Two', '--task', '~ATM-12', '--range', 'HEAD~1..HEAD', '--json'],
        root,
        AGENT,
      ).json();
      // For programs: the key, never with the mark
      assert.deepEqual(created.tasks, ['ATM-12']);
      assert.deepEqual(ok(['review', 'list', '--json'], root, HAKAN).json()[0].review.tasks, ['ATM-12']);
      for (const named of ['~ATM-12', 'ATM-12', 'atm-12', PROVISIONAL])
        assert.equal(ok(['review', 'list', '--task', named, '--json'], root, HAKAN).json().length, 1, named);
      ok(['review', 'submit', 'CR-1', '--approve'], root, HAKAN);
      const approved = ok(['task', 'details', '~ATM-12', '--json'], root, HAKAN).json()[0];
      assert.equal(approved.review_state, 'approved');
      // The agent takes the key as atm printed it, either way
      ok(['task', 'state', '~ATM-12', 'done', '--note', 'Approved in CR-1.'], root, AGENT);
      assert.equal(ok(['task', 'details', 'ATM-12', '--json'], root, HAKAN).json()[0].status, 'done');
    } finally {
      removeDir(root);
    }
  });

  test('a review an earlier atm wrote with ~KEY reads as the key; doctor names a decision its task never got', () => {
    const root = keyed();
    try {
      commitFile(root, 'a.txt', 'one\ntwo\n', 'the change');
      ok(['review', 'create', '--title', 'Two', '--task', 'ATM-12', '--range', 'HEAD~1..HEAD'], root, AGENT);
      // As an earlier atm left it: the task with its mark, the review approved, the task still waiting
      const folder = fs
        .readdirSync(path.join(root, '.agentic-task-manager/reviews'))
        .find((f) => f.startsWith('CR-1-'));
      const file = path.join(root, '.agentic-task-manager/reviews', folder, 'review.md');
      fs.writeFileSync(
        file,
        fs
          .readFileSync(file, 'utf8')
          .replace('tasks = ["ATM-12"]', 'tasks = ["~ATM-12"]')
          .replace('status = "open"', 'status = "approved"'),
      );
      assert.deepEqual(ok(['review', 'show', 'CR-1', '--json'], root, HAKAN).json().header.tasks, ['ATM-12']);
      const doctor = atm(['doctor'], root, HAKAN);
      assert.match(
        doctor.stdout + doctor.stderr,
        /ATM-12: CR-1 is approved but the task still waits for its review: `atm task review approve ATM-12` records the decision/,
      );
    } finally {
      removeDir(root);
    }
  });
});

describe('reviews in a project with submodules (ATM-583)', () => {
  test('a range that moves a submodule: the move is its entry, beside the files', () => {
    const { root, sub } = projectWithSubmodule();
    try {
      const lib = path.join(root, 'lib');
      commitFile(lib, 'b.txt', 'one\ntwo\n', 'sub change');
      git(['add', 'lib'], root);
      commitFile(root, 'a.txt', 'one\nTWO\nthree\n', 'move lib');
      ok(['review', 'create', '--title', 'Move lib', '--task', 'T-1', '--range', 'HEAD~1..HEAD'], root, AGENT);
      assert.deepEqual(roundOf(root, 'CR-1', 1), { submodule: null, files: ['a.txt', 'lib'] });
      assert.match(ok(['review', 'show', 'CR-1', '--round', '1'], root, HAKAN).stdout, /\+Subproject commit [0-9a-f]+/);
      // A comment on the move as a whole
      ok(['review', 'comment', 'CR-1', '--file', 'lib', '--body', 'Why this version?'], root, HAKAN);
      assert.match(ok(['review', 'comments', 'CR-1'], root, HAKAN).stdout, /^## lib · round 1/m);
    } finally {
      removeDir(root);
      removeDir(sub);
    }
  });

  test('commits made inside a submodule: run there, the review is the project’s, its files under the submodule', () => {
    const { root, sub } = projectWithSubmodule();
    try {
      const lib = path.join(root, 'lib');
      commitFile(lib, 'b.txt', 'one\ntwo\n', 'sub change');
      // From the submodule's folder: the project's task and review, the submodule's commits
      ok(['review', 'create', '--title', 'In lib', '--task', 'T-1', '--range', 'HEAD~1..HEAD'], lib, AGENT);
      assert.equal(task(root, 'T-1').status, 'in_review');
      assert.ok(fs.existsSync(path.join(root, '.agentic-task-manager/reviews')), 'kept in the project');
      assert.ok(!fs.existsSync(path.join(lib, '.agentic-task-manager')), 'nothing written in the submodule');
      assert.deepEqual(roundOf(root, 'CR-1', 1), { submodule: 'lib', files: ['lib/b.txt'] });
      const diff = ok(['review', 'show', 'CR-1', '--round', '1'], root, HAKAN).stdout;
      assert.match(diff, /diff --git a\/lib\/b\.txt b\/lib\/b\.txt[\s\S]*\+two/);

      // A comment on its lines, a fix in the submodule, the next round: the comment follows
      ok(
        ['review', 'comment', 'CR-1', '--file', 'lib/b.txt', '--lines', '2', '--quote', 'two', '--body', 'Say 2.'],
        root,
        HAKAN,
      );
      ok(['review', 'submit', 'CR-1', '--request-changes'], root, HAKAN);
      ok(['task', 'state', 'T-1', 'in_progress'], root, AGENT);
      commitFile(lib, 'b.txt', 'zero\none\ntwo\n', 'a line above');
      ok(['review', 'round', 'CR-1', '--range', 'HEAD~2..HEAD'], lib, AGENT);
      assert.deepEqual(roundOf(root, 'CR-1', 2), { submodule: 'lib', files: ['lib/b.txt'] });
      const between = ok(['review', 'show', 'CR-1', '--round', '2', '--against', '1'], root, HAKAN).stdout;
      assert.match(between, /\+zero/);
      assert.doesNotMatch(between, /\+two/);

      // A round of the project's own after one of the submodule's: shown, but the two don't compare
      commitFile(root, 'a.txt', 'one\nTWO\nthree\n', 'in the project');
      ok(['review', 'round', 'CR-1', '--range', 'HEAD~1..HEAD'], root, AGENT);
      const refused = atm(['review', 'show', 'CR-1', '--round', '3', '--against', '2'], root, HAKAN);
      assert.equal(refused.code, 2, refused.stderr);
      assert.match(refused.stderr, /round 2 is of the submodule lib and round 3 of the project/);
    } finally {
      removeDir(root);
      removeDir(sub);
    }
  });

  test('uncommitted changes in a submodule, and its files reviewed whole', () => {
    const { root, sub } = projectWithSubmodule();
    try {
      const lib = path.join(root, 'lib');
      fs.writeFileSync(path.join(lib, 'b.txt'), 'one\nnot committed\n');
      ok(['review', 'create', '--title', 'Working', '--task', 'T-1', '--uncommitted'], lib, AGENT);
      assert.deepEqual(roundOf(root, 'CR-1', 1), { submodule: 'lib', files: ['lib/b.txt'] });
      assert.match(ok(['review', 'show', 'CR-1', '--round', '1'], root, HAKAN).stdout, /\+not committed/);
      // From the project's folder, the project's own changes: the dirty submodule is one entry
      fs.writeFileSync(path.join(root, 'a.txt'), 'one\nchanged\n');
      ok(['review', 'create', '--title', 'Project', '--task', 'T-2', '--uncommitted'], root, AGENT);
      assert.equal(roundOf(root, 'CR-2', 1).submodule, null);
      // Files reviewed whole: a submodule's from its repository; with the project's, refused
      ok(['review', 'create', '--title', 'A file', '--file', 'lib/b.txt', '--draft'], root, AGENT);
      assert.deepEqual(roundOf(root, 'CR-3', 1), { submodule: 'lib', files: ['lib/b.txt'] });
      const mixed = atm(
        ['review', 'create', '--title', 'Both', '--file', 'lib/b.txt', '--file', 'a.txt', '--draft'],
        root,
        AGENT,
      );
      assert.equal(mixed.code, 2, mixed.stderr);
      assert.match(mixed.stderr, /in different repositories/);
    } finally {
      removeDir(root);
      removeDir(sub);
    }
  });

  test('a submodule with a task folder of its own is a project of its own', () => {
    const { root, sub } = projectWithSubmodule();
    try {
      const lib = path.join(root, 'lib');
      fs.mkdirSync(path.join(lib, '.agentic-task-manager'));
      fs.writeFileSync(
        path.join(lib, '.agentic-task-manager/tasks.json'),
        JSON.stringify({
          meta: { project: 'Lib' },
          tasks: [{ id: 'L-1', name: 'Its own', type: 'feature', status: 'in_progress' }],
        }),
      );
      const listed = ok(['task', 'list', '--json'], lib, HAKAN).json();
      assert.deepEqual(
        listed.map((t) => t.id),
        ['L-1'],
      );
    } finally {
      removeDir(root);
      removeDir(sub);
    }
  });
});

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
});
