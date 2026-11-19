// Task keys in code reviews (ATM-594): a review names its task by its key.
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

describe('a provisional task key in reviews (ATM-594)', () => {
  const PROVISIONAL = '0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8012';

  /** A project in task keys' format, in git: ATM-12 provisional and in progress (`blocker`), `a.txt` committed. */
  function keyed() {
    return makeProject({
      commit: true,
      files: {
        '.agentic-task-manager/tasks.json': {
          meta: {
            project: 'Keyed',
            format: 2,
            key: 'ATM',
            next_number: 13,
            review: 'blocker',
          },
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
