// The archive hint (CLI-5): when tasks.json is over meta.archive.threshold_kb or has more archivable tasks than
// threshold_tasks, `atm status` and `atm migrate` say so every time, and `atm done` at most once a day per clone.
import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { makeProject, removeDir, runAtm } from './helpers.mjs';

const NOW = '2026-05-01T12:00:00Z';
const finished = (n) =>
  Array.from({ length: n }, (_, i) => ({
    id: `T-${String(i + 1).padStart(3, '0')}`,
    status: 'done',
    completed_at: '2026-03-01T00:00:00Z',
  }));

function project(tasks, meta = {}) {
  return makeProject({ git: true, files: { '.agentic-task-manager/tasks.json': { meta, tasks } } });
}

describe('archive hint', () => {
  test('more archivable tasks than threshold_tasks: status and migrate suggest archiving', () => {
    const root = project(finished(4), { archive: { threshold_tasks: 3 } });
    try {
      const status = runAtm(['project', 'status'], root, { ATM_NOW: NOW });
      assert.match(status.stdout, /tasks\.json is \d+ KB with 4 finished tasks; `atm db archive` would move 4 of them/);
      const json = JSON.parse(runAtm(['project', 'status', '--json'], root, { ATM_NOW: NOW }).stdout);
      assert.deepEqual(
        [json.suggest.archive.finished, json.suggest.archive.archivable, json.suggest.archive.auto],
        [4, 4, true],
      );
      assert.match(runAtm(['db', 'migrate'], root, { ATM_NOW: NOW }).stdout, /`atm db archive` would move 4/);
      assert.equal(
        JSON.parse(runAtm(['db', 'migrate', '--json'], root, { ATM_NOW: NOW }).stdout).suggest.archive.archivable,
        4,
      );
    } finally {
      removeDir(root);
    }
  });

  test('meta.archive.auto false still shows the hint, marked for the skills to only relay it', () => {
    const root = project(finished(4), { archive: { threshold_tasks: 3, auto: false } });
    try {
      const json = JSON.parse(runAtm(['project', 'status', '--json'], root, { ATM_NOW: NOW }).stdout);
      assert.equal(json.suggest.archive.auto, false);
    } finally {
      removeDir(root);
    }
  });

  test('a file over threshold_kb suggests archiving even with few tasks', () => {
    const big = finished(2).map((t) => ({ ...t, description: 'x'.repeat(3000) }));
    const root = project(big, { archive: { threshold_kb: 4 } });
    try {
      assert.match(
        runAtm(['project', 'status'], root, { ATM_NOW: NOW }).stdout,
        /`atm db archive` would move 2 of them/,
      );
    } finally {
      removeDir(root);
    }
  });

  test('below the thresholds, or with nothing to move (all recent), there is no hint', () => {
    const recent = finished(4).map((t) => ({ ...t, completed_at: '2026-04-30T00:00:00Z' }));
    for (const [tasks, meta] of [
      [finished(4), {}],
      [recent, { archive: { threshold_tasks: 1 } }],
    ]) {
      const root = project(tasks, meta);
      try {
        assert.doesNotMatch(runAtm(['project', 'status'], root, { ATM_NOW: NOW }).stdout, /atm db archive/);
        assert.equal(
          JSON.parse(runAtm(['project', 'status', '--json'], root, { ATM_NOW: NOW }).stdout).suggest,
          undefined,
        );
      } finally {
        removeDir(root);
      }
    }
  });

  test('done shows it at most once a day per clone', () => {
    const tasks = [
      ...finished(4),
      { id: 'T-100', status: 'in_progress' },
      { id: 'T-101', status: 'in_progress' },
      { id: 'T-102', status: 'in_progress' },
    ];
    const root = project(tasks, { archive: { threshold_tasks: 3 } });
    try {
      const env = { ATM_AGENT: 'alice' };
      const first = runAtm(['task', 'state', 'T-100', 'done'], root, { ...env, ATM_NOW: NOW });
      assert.match(first.stdout, /`atm db archive` would move 4/);
      const second = runAtm(['task', 'state', 'T-101', 'done'], root, { ...env, ATM_NOW: '2026-05-01T18:00:00Z' });
      assert.doesNotMatch(second.stdout, /atm db archive/);
      // The next day it comes back once
      const nextDay = runAtm(['task', 'state', 'T-102', 'done'], root, { ...env, ATM_NOW: '2026-05-02T09:10:00Z' });
      assert.equal(nextDay.code, 0, nextDay.stderr);
      assert.match(nextDay.stdout, /`atm db archive` would move/);
    } finally {
      removeDir(root);
    }
  });
});
