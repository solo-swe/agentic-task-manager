// `atm review` (docs/code-review-design.md §3.9, ATM-566): code reviews recorded in the project, through the CLI as
// agents and people run it, with the tasks they review kept in step (ATM-565): a review from its opening to its
// decision, its rounds, comments, drafts and pokes.
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
      // Waiting on its reviewers (ATM-613), shown so; the file keeps writing `open`, as an older atm reads it (an app
      // or a clone on 0.40.0 sees the review), and reads `waiting` too
      assert.equal(opened.status, 'waiting');
      const reviewFile = path.join(
        root,
        '.agentic-task-manager/reviews',
        fs.readdirSync(path.join(root, '.agentic-task-manager/reviews')).find((f) => f.startsWith('CR-1-')),
        'review.md',
      );
      assert.match(fs.readFileSync(reviewFile, 'utf8'), /status = "open"/);
      fs.writeFileSync(
        reviewFile,
        fs.readFileSync(reviewFile, 'utf8').replace('status = "open"', 'status = "waiting"'),
      );
      assert.equal(ok(['review', 'show', 'CR-1', '--json'], root, HAKAN).json().header.status, 'waiting');
      fs.writeFileSync(
        reviewFile,
        fs.readFileSync(reviewFile, 'utf8').replace('status = "waiting"', 'status = "open"'),
      );
      // Open, in a filter, is every review not decided yet; waiting is its own status
      assert.equal(ok(['review', 'list', '--status', 'open', '--json'], root, HAKAN).json().length, 1);
      assert.equal(ok(['review', 'list', '--status', 'waiting', '--json'], root, HAKAN).json().length, 1);
      assert.equal(ok(['review', 'list', '--status', 'live', '--json'], root, HAKAN).json().length, 0);
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

      // Sent back, the review still takes the reviewer's words (ATM-597): a comment, and a decision's comment with a
      // message, which is kept; sent back already, it isn't sent back again (CR-2), and the task isn't changed again
      ok(['review', 'comment', 'CR-1', '--body', 'And the title too.'], root, HAKAN);
      const again = atm(['review', 'submit', 'CR-1', '--request-changes', '--body', 'Both, please.'], root, HAKAN);
      assert.equal(again.code, 1);
      assert.match(again.stderr, /sent back/);
      ok(['review', 'submit', 'CR-1', '--comment', '--body', 'Both, please.'], root, HAKAN);
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

  test('a review atm can’t read is listed as such beside the others, never left out without a word (ATM-617)', () => {
    const root = project();
    try {
      fs.writeFileSync(path.join(root, 'a.txt'), 'changed\n');
      ok(['review', 'create', '--title', 'Fine', '--task', 'T-1', '--uncommitted'], root, AGENT);
      ok(['review', 'create', '--title', 'Newer', '--task', 'T-2', '--uncommitted'], root, AGENT);
      const reviews = path.join(root, '.agentic-task-manager', 'reviews');
      const folder = fs.readdirSync(reviews).find((name) => name.startsWith('CR-2-'));
      const file = path.join(reviews, folder, 'review.md');
      fs.writeFileSync(file, fs.readFileSync(file, 'utf8').replace('open', 'frozen'));
      const listed = ok(['review', 'list'], root, HAKAN).stdout;
      assert.match(listed, /CR-1 .*Fine/);
      assert.match(
        listed,
        new RegExp(`CR-2 \\(${folder}\\): can't be read: .*frozen.*A newer atm may have written it`),
      );
      // With --json the list is still the array of what can be read, and stderr names the other
      const json = ok(['review', 'list', '--json'], root, HAKAN);
      assert.deepEqual(
        json.json().map((f) => f.review.reference),
        ['CR-1'],
      );
      assert.match(json.stderr, new RegExp(`atm: CR-2 \\(${folder}\\): can't be read`));
      // Whatever the filter: it has no status to filter by
      assert.match(ok(['review', 'list', '--status', 'closed'], root, HAKAN).stdout, /No reviews\.\nCR-2 /);
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
      assert.deepEqual([header.author, header.status], [AGENT.ATM_AGENT, 'waiting']);
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

  test('a later files round keeps the files the last one reviewed, and since round 1 is only what changed', () => {
    const root = project();
    try {
      fs.writeFileSync(path.join(root, 'b.txt'), 'bee\n');
      ok(['review', 'create', '--title', 'Two files', '--file', 'a.txt', '--file', 'b.txt'], root, AGENT);
      const a = fs.readFileSync(path.join(root, 'a.txt'), 'utf8');
      fs.writeFileSync(path.join(root, 'a.txt'), a.replace(/\n$/, ' changed\n'));
      assert.equal(ok(['review', 'round', 'CR-1', '--file', 'a.txt'], root, AGENT).stdout, 'CR-1: round 2\n');
      assert.match(ok(['review', 'show', 'CR-1'], root, HAKAN).stdout, /round 2 · files as they are · 2 file\(s\)/);
      const since = ok(['review', 'show', 'CR-1', '--round', '2', '--against', '1'], root, HAKAN).stdout;
      assert.match(since, /^\+.* changed$/m);
      assert.doesNotMatch(since, /b\.txt/);
      // A file gone since isn't kept
      fs.rmSync(path.join(root, 'b.txt'));
      assert.equal(ok(['review', 'round', 'CR-1', '--file', 'a.txt'], root, AGENT).stdout, 'CR-1: round 3\n');
      assert.match(ok(['review', 'show', 'CR-1'], root, HAKAN).stdout, /round 3 · files as they are · 1 file\(s\)/);
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
      assert.equal(missing.code, 2);
      assert.match(missing.stderr, /--summary-file none\.md: can't read it/);
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

  // ATM-631: never a review on disk beside a task that didn't move
  test('opening a review puts its task in review or refuses and writes nothing; a task whose own claim lapsed is taken back', () => {
    const root = makeProject({
      commit: true,
      files: {
        '.agentic-task-manager/tasks.json': {
          meta: { project: 'Reviews', review: 'blocker' },
          tasks: [
            { id: 'T-1', name: 'Mine', type: 'feature', status: 'todo' },
            { id: 'T-2', name: 'Theirs', type: 'feature', status: 'todo' },
            { id: 'T-3', name: 'Finished', type: 'feature', status: 'done' },
            { id: 'T-4', name: 'Not started', type: 'feature', status: 'todo' },
          ],
        },
        'a.txt': 'one\n',
      },
    });
    const OTHER = { ATM_AGENT: 'claude:other' };
    const refused = (taskId, env, pattern, code = 1) => {
      const r = atm(['review', 'create', '--title', 'x', '--task', taskId, '--uncommitted'], root, env);
      assert.equal(r.code, code, `${taskId}: ${r.stderr}`);
      assert.match(r.stderr, pattern);
      assert.equal(ok(['review', 'list', '--json'], root, HAKAN).json().length, 0, `${taskId}: a review was kept`);
    };
    try {
      fs.writeFileSync(path.join(root, 'a.txt'), 'changed\n');
      ok(['task', 'state', 'T-2', 'in_progress'], root, OTHER);
      refused('T-2', AGENT, /claimed by claude:other, who opens its review/, 4);
      assert.equal(task(root, 'T-2').status, 'in_progress');
      refused('T-3', AGENT, /it is done\. Reopen it \(atm task state T-3 todo\)/);
      refused('T-4', AGENT, /it is to do, and you didn't start it\. Start it first \(atm task state T-4 in_progress\)/);
      assert.equal(task(root, 'T-4').status, 'todo');

      // T-1 started with a lease of under a second; it lapses while the checks run, then its review is opened
      const tasksFile = path.join(root, '.agentic-task-manager/tasks.json');
      const data = JSON.parse(fs.readFileSync(tasksFile, 'utf8'));
      data.meta.lease_minutes = 0.01;
      fs.writeFileSync(tasksFile, JSON.stringify(data));
      ok(['task', 'state', 'T-1', 'in_progress'], root, AGENT);
      Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 1500);
      // The usual lease again, for the claim taken back (a lease is fixed when it is taken)
      delete data.meta.lease_minutes;
      fs.writeFileSync(
        tasksFile,
        JSON.stringify({
          ...JSON.parse(fs.readFileSync(tasksFile, 'utf8')),
          meta: data.meta,
        }),
      );
      const opened = ok(
        ['review', 'create', '--title', 'Mine', '--task', 'T-1', '--uncommitted', '--json'],
        root,
        AGENT,
      );
      assert.equal(opened.json().reference, 'CR-1');
      const t1 = task(root, 'T-1');
      assert.equal(t1.status, 'in_review');
      assert.match(JSON.stringify(t1.notes), /CR-1: Mine \(its claim had lapsed: taken back to open this review\)/);
    } finally {
      removeDir(root);
    }
  });

  // ATM-611: a poke is a person's nudge on the whole review, never open; an agent answers it, never makes one
  test('poke: a person nudges the review, with a message or none; an agent can’t; it never blocks an approval', () => {
    const root = project();
    try {
      fs.writeFileSync(path.join(root, 'a.txt'), 'changed\n');
      ok(
        ['review', 'create', '--title', 'Pokeable', '--task', 'T-1', '--uncommitted', '--reviewer', 'Hakan Serce'],
        root,
        AGENT,
      );
      assert.equal(ok(['review', 'poke', 'CR-1', '--body', 'Where are we?'], root, HAKAN).stdout, 'CR-1: poked (c1)\n');
      assert.deepEqual(ok(['review', 'poke', 'CR-1', '--json'], root, HAKAN).json(), { review: 'CR-1', poke: 'c2' });
      const refused = atm(['review', 'poke', 'CR-1'], root, AGENT);
      assert.equal(refused.code, 2);
      assert.match(refused.stderr, /a person pokes a review: an agent answers a poke/);
      // Pokes are review-level, marked, and never open
      const chains = ok(['review', 'comments', 'CR-1', '--json'], root, HAKAN).json();
      assert.deepEqual(
        chains.map((c) => [c.anchor.path, c.comments[0].poke, c.comments[0].resolved, c.comments[0].body]),
        [
          [null, true, null, 'Where are we?'],
          [null, true, null, ''],
        ],
      );
      assert.equal(ok(['review', 'comments', 'CR-1', '--open', '--json'], root, HAKAN).json().length, 0);
      // The agent answers with a reply; the reviewer approves with the pokes there
      ok(['review', 'reply', 'CR-1', 'c1', '--body', 'Round 2 is coming.'], root, AGENT);
      ok(['review', 'submit', 'CR-1', '--approve'], root, HAKAN);
      const late = atm(['review', 'poke', 'CR-1'], root, HAKAN);
      assert.equal(late.code, 1);
      assert.match(late.stderr, /CR-1 is approved: it can't be poked/);
      assert.equal(ok(['review', 'poke', 'CR-1', '-n'], root, HAKAN).stdout, '[dry-run] would poke CR-1\n');
    } finally {
      removeDir(root);
    }
  });

  // ATM-612: a person's drafts, sent together with their next decision; an agent comments at once
  test('draft: a person drafts comments and replies, sees only theirs, and sends them with a decision', () => {
    const root = project();
    try {
      fs.writeFileSync(path.join(root, 'a.txt'), 'changed\n');
      ok(
        ['review', 'create', '--title', 'Drafted', '--task', 'T-1', '--uncommitted', '--reviewer', 'Hakan Serce'],
        root,
        AGENT,
      );
      ok(['review', 'comment', 'CR-1', '--body', 'Done, I think.'], root, AGENT);
      const added = ok(
        ['review', 'draft', 'add', 'CR-1', '--file', 'a.txt', '--lines', '1', '--body', 'Why?'],
        root,
        HAKAN,
      );
      assert.equal(added.stdout, 'CR-1: drafted d1 (a.txt:1-1): Why?\n');
      assert.deepEqual(
        ok(['review', 'draft', 'add', 'CR-1', '--reply-to', 'c1', '--body', 'Not yet.', '--json'], root, HAKAN).json()
          .kind,
        'reply',
      );
      ok(['review', 'draft', 'add', 'CR-1', '--body', 'Gone soon.'], root, HAKAN);
      assert.equal(ok(['review', 'draft', 'remove', 'CR-1', 'd3'], root, HAKAN).stdout, 'CR-1: removed d3\n');
      assert.equal(
        ok(['review', 'draft', 'edit', 'CR-1', 'd1', '--body', 'Why this line?'], root, HAKAN).stdout,
        'CR-1: changed d1 (a.txt:1-1): Why this line?\n',
      );
      assert.equal(
        ok(['review', 'draft', 'list', 'CR-1'], root, HAKAN).stdout,
        'd1 (a.txt:1-1): Why this line?\nd2 (reply to c1): Not yet.\n',
      );
      // Only theirs: another person has none; an agent drafts nothing; a reply has no place of its own
      assert.equal(ok(['review', 'draft', 'list', 'CR-1'], root, { ATM_AGENT: 'Ana' }).stdout, 'CR-1: no drafts\n');
      const refused = atm(['review', 'draft', 'add', 'CR-1', '--body', 'x'], root, AGENT);
      assert.equal(refused.code, 2);
      assert.match(refused.stderr, /a person drafts a review's comments: an agent comments at once/);
      assert.equal(
        atm(['review', 'draft', 'add', 'CR-1', '--reply-to', 'c1', '--file', 'a.txt', '--body', 'x'], root, HAKAN).code,
        2,
      );
      assert.equal(
        ok(['review', 'draft', 'add', 'CR-1', '--body', 'x', '-n'], root, HAKAN).stdout,
        '[dry-run] would draft on CR-1\n',
      );
      // Not sent yet: the review has only the agent's comment
      assert.equal(ok(['review', 'comments', 'CR-1', '--json'], root, HAKAN).json().length, 1);
      // Approving with a drafted comment that would be open: refused, nothing sent
      const approve = atm(['review', 'submit', 'CR-1', '--approve'], root, HAKAN);
      assert.equal(approve.code, 1);
      assert.match(approve.stderr, /can't be approved while comments are open: c1, d1 \(your draft\)/);
      assert.equal(ok(['review', 'draft', 'list', 'CR-1', '--json'], root, HAKAN).json().drafts.length, 2);
      // Sent back, with them
      ok(['review', 'submit', 'CR-1', '--request-changes', '--body', 'Two things.'], root, HAKAN);
      const chains = ok(['review', 'comments', 'CR-1', '--json'], root, HAKAN).json();
      const bodies = chains.flatMap((c) => c.comments.flatMap((x) => [x.body, ...x.replies.map((r) => r.body)]));
      assert.deepEqual(bodies.sort(), ['Done, I think.', 'Not yet.', 'Two things.', 'Why this line?']);
      assert.equal(ok(['review', 'draft', 'list', 'CR-1'], root, HAKAN).stdout, 'CR-1: no drafts\n');
      // Sent back already: --comment alone publishes new drafts, and it stays sent back
      ok(['review', 'draft', 'add', 'CR-1', '--body', 'One more.'], root, HAKAN);
      const published = ok(['review', 'submit', 'CR-1', '--comment', '--json'], root, HAKAN).json();
      assert.equal(published.status, 'changes_requested');
      assert.ok(
        ok(['review', 'comments', 'CR-1', '--json'], root, HAKAN)
          .json()
          .some((c) => c.comments[0].body === 'One more.'),
      );
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

  // ATM-621: the task views atm serves carry where each of a task's code reviews stands (its badge says it from the
  // first load); one atm can't read says so; a task without reviews has no such field
  test('a task view says where each of its code reviews stands, unreadable for one atm can’t read', () => {
    const root = project();
    try {
      fs.writeFileSync(path.join(root, 'a.txt'), 'changed\n');
      ok(['review', 'create', '--title', 'First', '--task', 'T-1', '--uncommitted'], root, AGENT);
      const statuses = () =>
        Object.fromEntries(
          ok(['task', 'list', '--derived'], root, HAKAN)
            .json()
            .map((v) => [v.task.id, v.derived.review_statuses ?? null]),
        );
      const id = (n) => ok(['review', 'show', `CR-${n}`, '--json'], root, HAKAN).json().header.id;
      const first = id(1);
      assert.deepEqual(statuses(), { 'T-1': { [first]: 'waiting' }, 'T-2': null });
      ok(['review', 'submit', 'CR-1', '--approve'], root, HAKAN);
      assert.deepEqual(statuses()['T-1'], { [first]: 'approved' });
      // A second review of T-2, then its file made unreadable (ATM-617): its link says so
      ok(['review', 'create', '--title', 'Second', '--task', 'T-2', '--uncommitted'], root, AGENT);
      const second = id(2);
      const reviews = path.join(root, '.agentic-task-manager', 'reviews');
      const file = path.join(
        reviews,
        fs.readdirSync(reviews).find((n) => n.startsWith('CR-2-')),
        'review.md',
      );
      fs.writeFileSync(file, fs.readFileSync(file, 'utf8').replace('open', 'frozen'));
      assert.deepEqual(statuses()['T-2'], { [second]: 'unreadable' });
    } finally {
      removeDir(root);
    }
  });
});
