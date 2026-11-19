// `atm review` refusing what it must: wrong uses named, their exit codes, and --dry-run writing nothing.
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

describe('atm review: refusals', () => {
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
        [['review', 'submit', 'CR-1', '--comment'], 2, /needs its text, or drafts to send/],
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
      assert.deepEqual([review.header.status, review.chains.length], ['waiting', 0]);
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

  // ATM-622 (Hakan): a later round naming only the file that changed showed every other file as deleted since round 1
});
