// Code reviews in a project with git submodules (ATM-583).
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

describe('reviews in a project with submodules (ATM-583)', () => {
  test('a range that moves a submodule: the move is its entry, beside the files', () => {
    const { root, sub } = projectWithSubmodule();
    try {
      const lib = path.join(root, 'lib');
      commitFile(lib, 'b.txt', 'one\ntwo\n', 'sub change');
      git(['add', 'lib'], root);
      commitFile(root, 'a.txt', 'one\nTWO\nthree\n', 'move lib');
      ok(['review', 'create', '--title', 'Move lib', '--task', 'T-1', '--range', 'HEAD~1..HEAD'], root, AGENT);
      assert.deepEqual(roundOf(root, 'CR-1', 1), {
        submodule: null,
        files: ['a.txt', 'lib'],
      });
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
      assert.deepEqual(roundOf(root, 'CR-1', 1), {
        submodule: 'lib',
        files: ['lib/b.txt'],
      });
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
      assert.deepEqual(roundOf(root, 'CR-1', 2), {
        submodule: 'lib',
        files: ['lib/b.txt'],
      });
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
      assert.deepEqual(roundOf(root, 'CR-1', 1), {
        submodule: 'lib',
        files: ['lib/b.txt'],
      });
      assert.match(ok(['review', 'show', 'CR-1', '--round', '1'], root, HAKAN).stdout, /\+not committed/);
      // From the project's folder, the project's own changes: the dirty submodule is one entry
      fs.writeFileSync(path.join(root, 'a.txt'), 'one\nchanged\n');
      ok(['review', 'create', '--title', 'Project', '--task', 'T-2', '--uncommitted'], root, AGENT);
      assert.equal(roundOf(root, 'CR-2', 1).submodule, null);
      // Files reviewed whole: a submodule's from its repository; with the project's, refused
      ok(['review', 'create', '--title', 'A file', '--file', 'lib/b.txt', '--draft'], root, AGENT);
      assert.deepEqual(roundOf(root, 'CR-3', 1), {
        submodule: 'lib',
        files: ['lib/b.txt'],
      });
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
          tasks: [
            {
              id: 'L-1',
              name: 'Its own',
              type: 'feature',
              status: 'in_progress',
            },
          ],
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
