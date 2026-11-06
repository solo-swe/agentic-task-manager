// Claims and leases: who works on which task, shared across the worktrees of a clone, expiring when an agent stops
// renewing its lease. Time is controlled with ATM_NOW.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { describe, test } from 'node:test';
import { ATM, baseEnv, git, makeProject, removeDir, runAtm, tempDir } from './helpers.mjs';

const T0 = '2026-05-01T10:00:00Z';
const at = (minutes) => new Date(Date.parse(T0) + minutes * 60_000).toISOString().replace(/\.\d+Z$/, 'Z');
const task = (id, status = 'todo', deps = []) => ({
  id,
  name: id,
  phase: 0,
  type: 'feature',
  status,
  dependencies: deps,
});
const TASKS = { meta: { project: 'Claims' }, tasks: [task('T-001'), task('T-002'), task('T-003', 'todo', ['T-001'])] };

const tasksOf = (root) => JSON.parse(fs.readFileSync(path.join(root, '.agentic-task-manager/tasks.json'), 'utf8'));
const statusOf = (root, id) => tasksOf(root).tasks.find((t) => t.id === id);
const snapshot = (root) => JSON.parse(fs.readFileSync(path.join(root, '.agentic-task-manager/claims.json'), 'utf8'));

/** A committed repo with the task list, so worktrees get their own copy of it. */
function repo(tasks = TASKS) {
  return makeProject({ commit: true, files: { '.agentic-task-manager/tasks.json': tasks } });
}
const as = (agent, minutes = 0) => ({ ATM_AGENT: agent, ATM_NOW: at(minutes) });

describe('claims', () => {
  test('start claims the task: agent, branch, worktree, lease; done drops the claim', () => {
    const root = repo();
    try {
      const r = runAtm(['task', 'state', 'T-001', 'in_progress', '--json'], root, as('alice'));
      assert.equal(r.code, 0, r.stderr);
      const claim = JSON.parse(r.stdout).claim;
      assert.deepEqual(claim, {
        task: 'T-001',
        agent: 'alice',
        branch: 'main',
        worktree: root,
        claimed_at: T0,
        lease_expires_at: at(30),
      });
      // The claims store is in the git common directory; the task folder gets a snapshot for the dashboard
      const store = JSON.parse(fs.readFileSync(path.join(root, '.git/atm/claims.json'), 'utf8'));
      assert.deepEqual(store.claims['T-001'], claim);
      assert.deepEqual(snapshot(root).claims, [claim]);

      assert.match(runAtm(['claim', 'list'], root, as('bob', 5)).stdout, /^T-001 {2}alice {2}main {2}25m left/);
      assert.equal(runAtm(['task', 'state', 'T-001', 'done'], root, as('alice', 10)).code, 0);
      assert.deepEqual(JSON.parse(runAtm(['claim', 'list', '--json'], root, as('bob', 11)).stdout), []);
      assert.deepEqual(snapshot(root).claims, []);
    } finally {
      removeDir(root);
    }
  });

  test("another agent can't finish, release, fail or heartbeat a claimed task", () => {
    const root = repo();
    try {
      runAtm(['task', 'state', 'T-001', 'in_progress'], root, as('alice'));
      for (const [cmd, args] of [
        ['done', ['task', 'state', 'T-001', 'done']],
        ['release', ['task', 'state', 'T-001', 'todo']],
        ['fail', ['task', 'state', 'T-001', 'todo', '--reason', 'broken']],
        ['heartbeat', ['claim', 'heartbeat', 'T-001']],
      ]) {
        const r = runAtm(args, root, as('bob', 1));
        assert.equal(r.code, 4, `${cmd}: ${r.stderr}`);
        assert.match(r.stderr, new RegExp(`can't ${cmd} T-001: it is claimed by alice until ${at(30)}`));
      }
      assert.equal(statusOf(root, 'T-001').status, 'in_progress');
      // --agent on the command line identifies the holder too
      assert.equal(runAtm(['task', 'state', 'T-001', 'todo', '--agent', 'alice'], root, as('bob', 2)).code, 0);
    } finally {
      removeDir(root);
    }
  });

  test('heartbeat extends the lease from now; meta.lease_minutes sets its length', () => {
    const root = repo({ ...TASKS, meta: { project: 'Claims', lease_minutes: 10 } });
    try {
      runAtm(['task', 'state', 'T-001', 'in_progress'], root, as('alice'));
      const r = runAtm(['claim', 'heartbeat', 'T-001'], root, as('alice', 8));
      assert.equal(r.stdout, `T-001: lease extended to ${at(18)}\n`);
      assert.equal(runAtm(['claim', 'heartbeat', 'T-002'], root, as('alice', 8)).code, 4);
    } finally {
      removeDir(root);
    }
  });

  test('heartbeat --auto renews only claims last renewed ten minutes ago, silently, without touching tasks.json', () => {
    const root = repo();
    const store = () => JSON.parse(fs.readFileSync(path.join(root, '.git/atm/claims.json'), 'utf8'));
    try {
      runAtm(['task', 'state', 'T-001', 'in_progress'], root, as('alice'));
      runAtm(['task', 'state', 'T-002', 'in_progress'], root, as('bob'));
      const before = fs.readFileSync(path.join(root, '.agentic-task-manager/tasks.json'), 'utf8');
      const storeMtime = fs.statSync(path.join(root, '.git/atm/claims.json')).mtimeMs;

      // Fresh: nothing is written
      const fresh = runAtm(['claim', 'heartbeat', '--auto'], root, as('alice', 9));
      assert.deepEqual([fresh.code, fresh.stdout, fresh.stderr], [0, '', '']);
      assert.equal(fs.statSync(path.join(root, '.git/atm/claims.json')).mtimeMs, storeMtime);

      // Ten minutes after the claim: alice's lease is renewed, bob's is left alone
      const due = runAtm(['claim', 'heartbeat', '--auto'], root, as('alice', 10));
      assert.deepEqual([due.code, due.stdout, due.stderr], [0, '', '']);
      assert.equal(store().claims['T-001'].lease_expires_at, at(40));
      assert.equal(store().claims['T-001'].renewed_at, at(10));
      assert.equal(store().claims['T-002'].lease_expires_at, at(30));
      // The next renewal counts from the last one
      runAtm(['claim', 'heartbeat', '--auto'], root, as('alice', 15));
      assert.equal(store().claims['T-001'].lease_expires_at, at(40));
      assert.equal(fs.readFileSync(path.join(root, '.agentic-task-manager/tasks.json'), 'utf8'), before);
    } finally {
      removeDir(root);
    }
  });

  test('heartbeat --auto as a hook: the session and cwd come from stdin; it never fails', () => {
    const root = repo();
    const elsewhere = tempDir();
    try {
      runAtm(['task', 'state', 'T-001', 'in_progress'], root, { CLAUDE_CODE_SESSION_ID: 'abcdef1234', ATM_NOW: T0 });
      const hook = (input, cwd = elsewhere) =>
        spawnSync(ATM, ['claim', 'heartbeat', '--auto'], {
          cwd,
          input,
          encoding: 'utf8',
          env: { ...baseEnv(), ATM_NOW: at(12) },
        });
      const r = hook(JSON.stringify({ session_id: 'abcdef1234-rest', cwd: root, hook_event_name: 'PostToolUse' }));
      assert.deepEqual([r.status, r.stdout, r.stderr], [0, '', '']);
      const store = JSON.parse(fs.readFileSync(path.join(root, '.git/atm/claims.json'), 'utf8'));
      assert.equal(store.claims['T-001'].lease_expires_at, at(42));
      // Outside a project, with garbage on stdin: still exit 0, nothing printed
      for (const input of ['not json', '']) {
        const quiet = hook(input);
        assert.deepEqual([quiet.status, quiet.stdout, quiet.stderr], [0, '', '']);
      }
      assert.equal(runAtm(['claim', 'heartbeat'], root, as('alice')).code, 2);
    } finally {
      removeDir(root);
      removeDir(elsewhere);
    }
  });

  test('an expired lease frees the task: back to todo, with a note on who held it', () => {
    const root = repo();
    try {
      runAtm(['task', 'state', 'T-001', 'in_progress'], root, as('alice'));
      const r = runAtm(['claim', 'list', '--json'], root, as('bob', 31));
      assert.deepEqual(JSON.parse(r.stdout), []);
      const t = statusOf(root, 'T-001');
      assert.equal(t.status, 'todo');
      assert.equal(t.started_at, undefined);
      assert.equal(t.started_by, undefined);
      assert.deepEqual(t.notes, [{ at: at(31), kind: 'failure', text: 'lease expired (alice)' }]);
      // Now bob can take it; alice's late heartbeat is told what happened
      assert.equal(runAtm(['task', 'state', 'T-001', 'in_progress'], root, as('bob', 32)).code, 0);
      const late = runAtm(['claim', 'heartbeat', 'T-001'], root, as('alice', 33));
      assert.equal(late.code, 4);
      assert.match(late.stderr, /claimed by bob/);
    } finally {
      removeDir(root);
    }
  });

  test('worktrees of one clone share claims', () => {
    const root = repo();
    const wt = path.join(tempDir(), 'wt2');
    try {
      git(['worktree', 'add', '-q', '-b', 'agent-2', wt], root);
      runAtm(['task', 'state', 'T-001', 'in_progress'], root, as('alice'));
      // The second worktree has its own tasks.json (T-001 still todo there) but sees alice's claim
      assert.equal(statusOf(wt, 'T-001').status, 'todo');
      const r = runAtm(['task', 'state', 'T-001', 'in_progress'], wt, as('bob', 1));
      assert.equal(r.code, 4);
      assert.match(r.stderr, /claimed by alice/);
      assert.equal(runAtm(['task', 'state', 'T-002', 'in_progress'], wt, as('bob', 1)).code, 0);
      const live = JSON.parse(runAtm(['claim', 'list', '--json'], root, as('carol', 2)).stdout);
      assert.deepEqual(
        live.map((c) => [c.task, c.agent, c.branch, c.worktree]),
        [
          ['T-001', 'alice', 'main', root],
          ['T-002', 'bob', 'agent-2', wt],
        ],
      );
    } finally {
      removeDir(root);
      removeDir(path.dirname(wt));
    }
  });

  test('subagents of one session in different worktrees get identities of their own (T-202)', () => {
    const root = repo();
    const wt = path.join(tempDir(), 'wt2');
    const session = (minutes) => ({ CLAUDE_CODE_SESSION_ID: 'abcdef1234567890', ATM_NOW: at(minutes) });
    const hook = (cwd, minutes) =>
      spawnSync(ATM, ['claim', 'heartbeat', '--auto'], {
        cwd,
        input: JSON.stringify({ session_id: 'abcdef1234567890', cwd, hook_event_name: 'PostToolUse' }),
        encoding: 'utf8',
        env: { ...baseEnv(), ATM_NOW: at(minutes) },
      });
    const claims = () => JSON.parse(fs.readFileSync(path.join(root, '.git/atm/claims.json'), 'utf8')).claims;
    try {
      git(['worktree', 'add', '-q', '-b', 'agent-2', wt], root);
      // The main worktree keeps the plain session identity; the linked one adds its folder
      assert.equal(
        JSON.parse(runAtm(['task', 'state', 'T-001', 'in_progress', '--json'], root, session(0)).stdout).claim.agent,
        'claude:abcdef12',
      );
      const linked = JSON.parse(runAtm(['task', 'state', 'T-002', 'in_progress', '--json'], wt, session(0)).stdout);
      assert.equal(linked.claim.agent, 'claude:abcdef12@wt2');
      assert.equal(linked.started_by, 'claude:abcdef12@wt2');

      // Neither can finish or release the other's task
      const steal = runAtm(['task', 'state', 'T-001', 'done'], wt, session(1));
      assert.equal(steal.code, 4);
      assert.match(steal.stderr, /claimed by claude:abcdef12 until/);
      assert.equal(runAtm(['task', 'state', 'T-002', 'todo'], root, session(1)).code, 4);

      // The hook in the linked worktree renews only that worktree's claim
      assert.equal(hook(wt, 12).status, 0);
      assert.deepEqual([claims()['T-001'].lease_expires_at, claims()['T-002'].lease_expires_at], [at(30), at(42)]);
      assert.equal(hook(root, 13).status, 0);
      assert.equal(claims()['T-001'].lease_expires_at, at(43));
    } finally {
      removeDir(root);
      removeDir(path.dirname(wt));
    }
  });

  describe('claim hold keeps the claim alive while its shell runs, and runs nothing (T-437)', () => {
    const storeOf = (root) => path.join(root, '.git/atm/claims.json');
    const claimOf = (root, id) => JSON.parse(fs.readFileSync(storeOf(root), 'utf8')).claims[id];
    /** Remove the claim's renewed_at: the time is fixed (ATM_NOW), so one back in the store proves a renewal since. */
    const forget = (root) => {
      const s = JSON.parse(fs.readFileSync(storeOf(root), 'utf8'));
      delete s.claims['T-001'].renewed_at;
      fs.writeFileSync(storeOf(root), JSON.stringify(s));
    };
    const alive = (pid) => {
      try {
        process.kill(pid, 0);
        return true;
      } catch {
        return false;
      }
    };
    const until = (check, ms = 5000) => {
      const end = Date.now() + ms;
      while (Date.now() < end) {
        if (check()) return true;
        spawnSync('sleep', ['0.1']);
      }
      return check();
    };
    /** `sh -c 'atm claim hold T-001 <options> --json > held.json; <then>'`, as an agent's Bash call runs it: the shell is
     * the watcher's to follow. Returns {code, held} once the shell has ended. */
    const shell = (root, options, then, minutes) => {
      const held = path.join(root, 'held.json');
      const line = `"$ATM" claim hold T-001 ${options} --json > '${held}' && ${then}`;
      const r = spawnSync('sh', ['-c', line], {
        cwd: root,
        encoding: 'utf8',
        env: { ...baseEnv(), ATM, ...as('alice', minutes) },
      });
      return {
        code: r.status,
        stderr: r.stderr,
        held: fs.existsSync(held) ? JSON.parse(fs.readFileSync(held, 'utf8') || 'null') : null,
      };
    };

    test('it renews while the shell lives, and its watcher ends with the shell', () => {
      const root = repo();
      try {
        runAtm(['task', 'state', 'T-001', 'in_progress'], root, as('alice'));
        // The shell forgets the renewal hold made, then waits: a renewal while it waits puts renewed_at back
        const forgetIt = `node -e "const fs=require('fs');const f='${storeOf(root)}';const s=JSON.parse(fs.readFileSync(f,'utf8'));delete s.claims['T-001'].renewed_at;fs.writeFileSync(f,JSON.stringify(s))"`;
        const r = shell(root, '--every 0.3', `${forgetIt} && sleep 1.5`, 20);
        assert.equal(r.code, 0, r.stderr);
        assert.equal(r.held.task, 'T-001');
        assert.equal(r.held.claim.lease_expires_at, at(50));
        assert.equal(claimOf(root, 'T-001').renewed_at, at(20));
        // The shell is gone: so is the watcher, within one interval
        assert.ok(
          until(() => !alive(r.held.watcher)),
          'the watcher should end with its shell',
        );
      } finally {
        removeDir(root);
      }
    });

    test('it stops at --max, and when the claim is released', () => {
      const root = repo();
      try {
        runAtm(['task', 'state', 'T-001', 'in_progress'], root, as('alice'));
        // --max 0.01 (0.6 s) while the shell waits 2 s: the watcher stops first
        const capped = shell(root, '--every 0.2 --max 0.01', 'sleep 2', 1);
        assert.equal(capped.code, 0, capped.stderr);
        assert.ok(!alive(capped.held.watcher), 'the watcher should stop at --max');
        // Released while the shell still waits: the next renewal fails, and the watcher ends
        const released = shell(root, '--every 0.2', `"$ATM" task state T-001 todo && sleep 1.5`, 2);
        assert.equal(released.code, 0, released.stderr);
        assert.equal(claimOf(root, 'T-001'), undefined);
        assert.ok(until(() => !alive(released.held.watcher)));
      } finally {
        removeDir(root);
      }
    });

    test('someone else’s claim, or none, is refused before any watcher starts; --max is bounded', () => {
      const root = repo();
      try {
        runAtm(['task', 'state', 'T-001', 'in_progress'], root, as('alice'));
        const bob = runAtm(['claim', 'hold', 'T-001'], root, as('bob', 1));
        assert.equal(bob.code, 4);
        assert.match(bob.stderr, /claimed by alice/);
        assert.equal(runAtm(['claim', 'hold', 'T-002'], root, as('alice', 1)).code, 4);
        for (const max of ['0', '721'])
          assert.equal(runAtm(['claim', 'hold', 'T-001', '--max', max], root, as('alice', 1)).code, 2);
        forget(root);
        assert.equal(claimOf(root, 'T-001').renewed_at, undefined);
      } finally {
        removeDir(root);
      }
    });
  });

  test('a task blocked or in review in one worktree waits in all of them, and is decided from any (T-204)', () => {
    const root = repo();
    const wt = path.join(tempDir(), 'wt2');
    const store = () => JSON.parse(fs.readFileSync(path.join(root, '.git/atm/claims.json'), 'utf8'));
    const notes = (dir, id) => (statusOf(dir, id).notes || []).map((n) => [n.kind, n.text]);
    try {
      git(['worktree', 'add', '-q', '-b', 'agent-2', wt], root);
      // alice blocks T-001 in the main worktree; her claim is released, but the task isn't handed out elsewhere
      runAtm(['task', 'state', 'T-001', 'in_progress'], root, as('alice'));
      const block = ['task', 'state', 'T-001', 'blocked', '--on', 'user', '--question', 'Which provider?'];
      assert.equal(runAtm(block, root, as('alice', 1)).code, 0);
      assert.deepEqual(
        [store().waiting['T-001'].status, store().waiting['T-001'].questions, store().waiting['T-001'].branch],
        ['blocked', ['Which provider?'], 'main'],
      );
      assert.equal(JSON.parse(runAtm(['loop', 'next', '--json'], wt, as('bob', 2)).stdout).task.id, 'T-002');
      runAtm(['loop', 'next', '--claim'], wt, as('bob', 2));
      const none = runAtm(['loop', 'next'], wt, as('bob', 3));
      assert.equal(none.code, 3);
      assert.match(none.stdout, /blocked or in review in another worktree: T-001/);
      // atm status there lists it as blocked, in the other worktree
      assert.match(
        runAtm(['project', 'status'], wt, as('bob', 3)).stdout,
        /blocked: T-001 T-001 \(on user\) \[in another worktree, main\]/,
      );

      // The user answers in the other worktree: the answer counts there at once ...
      assert.equal(statusOf(wt, 'T-001').status, 'todo');
      const answer = runAtm(['task', 'state', 'T-001', 'todo', '--answer', 'Postmark'], wt, as('carol', 4));
      assert.equal(answer.code, 0, answer.stderr);
      assert.deepEqual(notes(wt, 'T-001'), [['answer', 'Which provider? → Postmark']]);
      assert.equal(store().waiting['T-001'], undefined);
      assert.equal(store().resolved['T-001'].worktree, root);
      // ... and in the main worktree with its next command, which also gives the task out again
      assert.equal(statusOf(root, 'T-001').status, 'blocked');
      assert.equal(JSON.parse(runAtm(['loop', 'next', '--json'], root, as('dave', 5)).stdout).task.id, 'T-001');
      assert.equal(statusOf(root, 'T-001').status, 'todo');
      assert.deepEqual(notes(root, 'T-001'), [['answer', 'Which provider? → Postmark']]);
      assert.equal(store().resolved['T-001'], undefined);

      // bob sends T-002 for review in his worktree; erin approves it there, and it is passed from the main worktree:
      // the shared record carries the decision, so the guard on done from review holds there too (T-438)
      assert.equal(runAtm(['task', 'state', 'T-002', 'in_review', '--note', 'check it'], wt, as('bob', 6)).code, 0);
      assert.equal(store().waiting['T-002'].status, 'in_review');
      assert.equal(runAtm(['task', 'state', 'T-002', 'done'], root, as('erin', 7)).code, 1); // undecided
      assert.equal(runAtm(['task', 'review', 'approve', 'T-002'], wt, as('erin', 7)).code, 0);
      assert.equal(store().waiting['T-002'].review_state, 'approved');
      const pass = runAtm(['task', 'state', 'T-002', 'done'], root, as('erin', 7));
      assert.equal(pass.code, 0, pass.stderr);
      assert.equal(statusOf(root, 'T-002').status, 'done');
      runAtm(['loop', 'next', '--json'], wt, as('bob', 8));
      assert.equal(statusOf(wt, 'T-002').status, 'done');
      assert.deepEqual(Object.keys(store().waiting), []);
      assert.deepEqual(Object.keys(store().resolved), []);
    } finally {
      removeDir(root);
      removeDir(path.dirname(wt));
    }
  });

  test('a task done in one worktree is not handed out again in another before the branches merge', () => {
    const root = repo();
    const wt = path.join(tempDir(), 'wt2');
    try {
      git(['worktree', 'add', '-q', '-b', 'agent-2', wt], root);
      runAtm(['task', 'state', 'T-001', 'in_progress'], root, as('alice'));
      runAtm(['task', 'state', 'T-001', 'done'], root, as('alice', 5));
      // In the other worktree T-001 is still todo, and unclaimed, but it is done
      assert.equal(statusOf(wt, 'T-001').status, 'todo');
      const next = JSON.parse(runAtm(['loop', 'next', '--json'], wt, as('bob', 6)).stdout);
      assert.equal(next.task.id, 'T-002');
      // T-003 depends on T-001, which isn't done in this worktree's file yet, so nothing else is available
      runAtm(['loop', 'next', '--claim'], wt, as('bob', 6));
      const none = runAtm(['loop', 'next'], wt, as('bob', 7));
      assert.equal(none.code, 3);
      assert.match(none.stdout, /done in another worktree, not merged yet: T-001/);
      // Starting it explicitly is still possible (deliberate rework)
      assert.equal(runAtm(['task', 'state', 'T-001', 'in_progress'], wt, as('bob', 8)).code, 0);
    } finally {
      removeDir(root);
      removeDir(path.dirname(wt));
    }
  });

  test('outside git, claims are kept in the task folder', () => {
    const root = makeProject({ files: { '.agentic-task-manager/tasks.json': TASKS } });
    try {
      assert.equal(runAtm(['task', 'state', 'T-002', 'in_progress'], root, as('alice')).code, 0);
      const store = JSON.parse(fs.readFileSync(path.join(root, '.agentic-task-manager/claims-store.json'), 'utf8'));
      assert.equal(store.claims['T-002'].agent, 'alice');
      assert.equal(store.claims['T-002'].branch, null);
      assert.equal(runAtm(['task', 'state', 'T-002', 'in_progress'], root, as('bob')).code, 4, 'claimed by alice');
    } finally {
      removeDir(root);
    }
  });

  test('the default identity is this Claude Code session, else the git user, else user@host', () => {
    const root = repo();
    const outside = makeProject({ files: { '.agentic-task-manager/tasks.json': TASKS } });
    try {
      const session = JSON.parse(
        runAtm(['task', 'state', 'T-001', 'in_progress', '--json'], root, {
          CLAUDE_CODE_SESSION_ID: 'abcdef1234567890',
        }).stdout,
      );
      assert.equal(session.claim.agent, 'claude:abcdef12');
      assert.equal(session.started_by, 'claude:abcdef12');
      // A person in a terminal: their git name (the test repository's is "atm test")
      const person = JSON.parse(runAtm(['task', 'state', 'T-002', 'in_progress', '--json'], root).stdout);
      assert.equal(person.claim.agent, 'atm test');
      assert.equal(person.started_by, 'atm test');
      // Outside git (and with no global git name in the test environment's HOME), user@host
      const env = { HOME: outside, GIT_CONFIG_NOSYSTEM: '1', XDG_CONFIG_HOME: outside };
      const plain = JSON.parse(runAtm(['task', 'state', 'T-002', 'in_progress', '--json'], outside, env).stdout);
      assert.match(plain.claim.agent, /^[^@:]+@[^:]+$/);
    } finally {
      removeDir(root);
      removeDir(outside);
    }
  });
});
