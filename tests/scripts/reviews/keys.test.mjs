// Task keys in code reviews (ATM-594): a review names its task by its key.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
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

  // ATM-630 (Hakan): a review names its task by the key it had when the review was opened. Once a merge moved the
  // task's number (ADR-009: the old key its alias), atm still finds the task from the review, and the review from the
  // task: listed by either key, the agent told of comments on it, the approval reaching it, done accepted
  test('a task whose number moved at a merge keeps its review: listed by either key, heard, approved, done', () => {
    const root = keyed();
    try {
      commitFile(root, 'a.txt', 'one\ntwo\n', 'the change');
      ok(['review', 'create', '--title', 'Two', '--task', 'ATM-12', '--range', 'HEAD~1..HEAD'], root, AGENT);
      // The first watch starts from now: nothing to tell yet
      assert.equal(atm(['review', 'watch', '--once', '--timeout', '1s'], root, AGENT).code, 3);
      // A merge moved it: number 14 now, ATM-12 its alias (as the merge driver writes it); the review still says ATM-12
      const file = path.join(root, '.agentic-task-manager/tasks.json');
      const data = JSON.parse(fs.readFileSync(file, 'utf8'));
      data.meta.next_number = 15;
      const moved = data.tasks.find((t) => t.id === PROVISIONAL);
      Object.assign(moved, { number: 14, aliases: ['ATM-12'] });
      delete moved.key_final;
      fs.writeFileSync(file, JSON.stringify(data, null, 2));
      assert.deepEqual(ok(['review', 'show', 'CR-1', '--json'], root, HAKAN).json().header.tasks, ['ATM-12']);
      // Listed by its key now, and by the key it had
      for (const named of ['ATM-14', 'ATM-12', PROVISIONAL])
        assert.equal(ok(['review', 'list', '--task', named, '--json'], root, HAKAN).json().length, 1, named);
      // A comment on it is the agent's to hear, about the task by its key now
      ok(['review', 'comment', 'CR-1', '--file', 'a.txt', '--lines', '2', '--body', 'Why two?'], root, HAKAN);
      const heard = atm(['review', 'watch', '--once', '--json', '--timeout', '10s'], root, AGENT);
      assert.equal(heard.code, 0, heard.stderr);
      const told = JSON.parse(heard.stdout.trim().split('\n')[0]);
      assert.deepEqual([told.body, told.task], ['Why two?', 'ATM-14']);
      // Resolved and approved: the approval reaches the task, which is done with no decision recorded by hand
      const comment = ok(['review', 'comments', 'CR-1', '--json'], root, HAKAN).json()[0].comments[0].id;
      ok(['review', 'resolve', 'CR-1', comment], root, HAKAN);
      ok(['review', 'submit', 'CR-1', '--approve'], root, HAKAN);
      assert.equal(task(root, 'ATM-14').review_state, 'approved');
      ok(['task', 'state', 'ATM-14', 'done', '--note', 'Approved in CR-1.'], root, AGENT);
      assert.equal(task(root, 'ATM-14').status, 'done');
    } finally {
      removeDir(root);
    }
  });
});

// ATM-630, CR-55 round 2: a real clash (ADR-009 §3). Two branches each made ATM-12, each with a review on it; the merge
// driver moved one to ATM-13, its old key an alias, while the other still holds ATM-12. Both reviews say ATM-12, and
// each still finds its own task, read from tasks.json at the review's own commit.
describe('two branches made ATM-12, each reviewed; a merge moved one (ATM-630)', () => {
  const OLDER = '0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8012';
  const NEWER = '0193a5b3-7c1d-7e2f-8a3b-4c5d6e7f8012';
  const OTHER = { ATM_AGENT: 'claude:other' };
  const FILE = '.agentic-task-manager/tasks.json';

  /** `name`'s task, ATM-12 in progress by `by`, on the branch checked out, with its change and its review, committed */
  function reviewedTask(root, { id, name, file, by }) {
    const data = JSON.parse(fs.readFileSync(path.join(root, FILE), 'utf8'));
    data.meta.next_number = 13;
    data.tasks.push({ id, number: 12, name, type: 'feature', status: 'in_progress', started_by: by.ATM_AGENT });
    fs.writeFileSync(path.join(root, FILE), JSON.stringify(data, null, 2) + '\n');
    commitFile(root, file, 'one\ntwo\n', `feat(ATM-12): ${name}`);
    ok(['review', 'create', '--title', name, '--task', 'ATM-12', '--range', 'HEAD~1..HEAD'], root, by);
    // As an agent commits it: the tasks and the review, not atm's runtime files (claims)
    git(['add', FILE, '.agentic-task-manager/reviews'], root);
    git(['commit', '-q', '-m', `review ${name}`], root);
  }

  /**
   * The older task ("On a branch", the agent's) on branch `a`, the newer ("On main", another agent's) on main, main
   * merged into `a` through atm's driver. With a remote, main pushed: the task not on it moves (the older). Without
   * one, the newer moves. The project's folder.
   */
  function clash({ remote }) {
    const root = makeProject({
      commit: true,
      files: {
        [FILE]: { meta: { project: 'Clash', format: 2, key: 'ATM', next_number: 12, review: 'blocker' }, tasks: [] },
      },
    });
    ok(['db', 'init'], root, HAKAN);
    git(['add', '-A'], root);
    git(['commit', '-q', '-m', 'atm init'], root);
    if (remote) {
      fs.mkdirSync(remote);
      git(['init', '-q', '--bare', remote], root);
      git(['remote', 'add', 'origin', remote], root);
      git(['push', '-q', '-u', 'origin', 'main'], root);
      git(['remote', 'set-head', 'origin', 'main'], root);
    }
    git(['switch', '-q', '-c', 'a'], root);
    reviewedTask(root, { id: OLDER, name: 'On a branch', file: 'a.txt', by: AGENT });
    git(['switch', '-q', 'main'], root);
    reviewedTask(root, { id: NEWER, name: 'On main', file: 'b.txt', by: OTHER });
    if (remote) git(['push', '-q', 'origin', 'main'], root);
    git(['switch', '-q', 'a'], root);
    const merged = spawnSync('git', ['merge', '--no-edit', 'main'], { cwd: root, encoding: 'utf8' });
    assert.equal(merged.status, 0, merged.stdout + merged.stderr);
    return root;
  }

  /** The reference of the review titled `title` */
  const reference = (root, title) =>
    ok(['review', 'list', '--json'], root, HAKAN)
      .json()
      .find((r) => r.review.title === title).review.reference;

  /** The titles of the reviews `review list --task <key>` lists */
  const listed = (root, key) =>
    ok(['review', 'list', '--task', key, '--json'], root, HAKAN)
      .json()
      .map((r) => r.review.title)
      .sort();

  for (const [how, remote, moved, stayed] of [
    ['main pushed: the task not on it moves', true, 'On a branch', 'On main'],
    ['no remote: the newer task moves', false, 'On main', 'On a branch'],
  ]) {
    test(`${how}; each review still finds its own task: listed, heard, approved`, () => {
      const scratch = fs.mkdtempSync(path.join(os.tmpdir(), 'atm-clash-'));
      const root = clash({ remote: remote && path.join(scratch, 'remote.git') });
      try {
        const keyOf = (name) =>
          ok(['task', 'list', '--json'], root, HAKAN)
            .json()
            .find((t) => t.name === name).key;
        // The merge moved one: ATM-13, its old key ATM-12 an alias; both reviews still say ATM-12
        assert.deepEqual([keyOf(moved), keyOf(stayed)], ['ATM-13', 'ATM-12']);
        for (const title of [moved, stayed])
          assert.deepEqual(ok(['review', 'show', reference(root, title), '--json'], root, HAKAN).json().header.tasks, [
            'ATM-12',
          ]);
        // Listed under its own task, by either key it answers to, and never under the other
        assert.deepEqual(listed(root, 'ATM-13'), [moved]);
        assert.deepEqual(listed(root, 'ATM-12'), [stayed]);
        // A comment on the moved task's review reaches its agent, about its key now
        const agent = moved === 'On a branch' ? AGENT : OTHER;
        assert.equal(atm(['review', 'watch', '--once', '--timeout', '1s'], root, agent).code, 3);
        const review = reference(root, moved);
        const file = moved === 'On a branch' ? 'a.txt' : 'b.txt';
        ok(['review', 'comment', review, '--file', file, '--lines', '2', '--body', 'Why two?'], root, HAKAN);
        const heard = atm(['review', 'watch', '--once', '--json', '--timeout', '10s'], root, agent);
        assert.equal(heard.code, 0, heard.stderr);
        const told = JSON.parse(heard.stdout.trim().split('\n')[0]);
        assert.deepEqual([told.review, told.body, told.task], [review, 'Why two?', 'ATM-13']);
        // Its approval reaches the moved task, not the one holding ATM-12 now
        const comment = ok(['review', 'comments', review, '--json'], root, HAKAN).json()[0].comments[0].id;
        ok(['review', 'resolve', review, comment], root, HAKAN);
        ok(['review', 'submit', review, '--approve'], root, HAKAN);
        assert.equal(task(root, 'ATM-13').review_state, 'approved');
        assert.notEqual(task(root, 'ATM-12').review_state, 'approved');
      } finally {
        removeDir(root);
        removeDir(scratch);
      }
    });
  }
});
