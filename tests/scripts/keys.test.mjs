// Task keys (ADR-007, T-540): `atm db migrate --keys` turns a task file of IDs (T-527) into one stored by UUID and
// shown by the project's key (ATM-527), and `atm project key set` changes the key. What a scenario can't say with one
// fixed output: the UUIDs are worked out (the same in every clone), and this repository's own file must come through
// whole.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { test } from 'node:test';
import { REPO, makeProject, removeDir, runAtm } from './helpers.mjs';

const DIR = '.agentic-task-manager';
const FILE = `${DIR}/tasks.json`;
const ARCHIVE = `${DIR}/tasks-archive.json`;
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

const read = (root, file) => JSON.parse(fs.readFileSync(path.join(root, file), 'utf8'));

test('migrate --keys: UUIDs, numbers, references and claims; the same in every clone; then key set twice', () => {
  const files = {
    [FILE]: {
      meta: { id: '3f2a9c1e', project: 'Learn Forever Blog', next_id: 9 },
      tasks: [
        { id: 'T-007', name: 'seven', type: 'feature', status: 'todo', dependencies: ['T-001'], related: ['T-P1-03'] },
        { id: 'T-P1-03', name: 'odd', type: 'feature', status: 'todo' },
      ],
    },
    [ARCHIVE]: { meta: {}, tasks: [{ id: 'T-001', name: 'one', type: 'feature', status: 'done' }] },
  };
  const roots = [makeProject({ files, git: true, commit: true }), makeProject({ files, git: true, commit: true })];
  try {
    const [root, clone] = roots;
    const r = runAtm(['db', 'migrate', '--keys', '--json'], root);
    assert.equal(r.code, 0, r.stderr);
    const keys = JSON.parse(r.stdout).task_keys;
    assert.deepEqual(keys, {
      key: 'LFB',
      old_prefix: 'T',
      tasks: 3,
      archived: 1,
      renumbered: [{ from: 'T-P1-03', number: 9 }],
    });
    const doc = read(root, FILE);
    const archived = read(root, ARCHIVE);
    assert.equal(doc.meta.format, 2);
    assert.equal(doc.meta.key, 'LFB');
    assert.deepEqual(doc.meta.key_history, ['T']);
    assert.equal(doc.meta.next_number, 10);
    assert.equal(doc.meta.next_id, undefined);
    const [seven, odd] = doc.tasks;
    const one = archived.tasks[0];
    for (const t of [seven, odd, one]) {
      assert.match(t.id, UUID);
      assert.equal(t.key_final, true);
    }
    assert.deepEqual([seven.number, odd.number, one.number], [7, 9, 1]);
    assert.deepEqual(odd.aliases, ['T-P1-03']);
    assert.deepEqual(seven.dependencies, [one.id]);
    assert.deepEqual(seven.related, [odd.id]);

    // Another clone of the same file migrates to the same UUIDs, so their branches merge task by task
    assert.equal(runAtm(['db', 'migrate', '--keys'], clone).code, 0);
    assert.deepEqual(read(clone, FILE).tasks, doc.tasks);

    // Again: nothing more to do
    const again = runAtm(['db', 'migrate', '--keys', '--json'], root);
    assert.equal(JSON.parse(again.stdout).task_keys, null);

    // The key changes; the old ones are kept, in order, so their keys still find the tasks
    const set = runAtm(['project', 'key', 'set', 'Blog', '--json'], root);
    assert.equal(set.code, 0, set.stderr);
    assert.deepEqual(JSON.parse(set.stdout), { from: 'LFB', to: 'Blog', history: ['T', 'LFB'], also_used_by: [] });
    assert.equal(runAtm(['project', 'key', 'set', 'LFB'], root).code, 0);
    const show = JSON.parse(runAtm(['project', 'key', 'show', '--json'], root).stdout);
    assert.deepEqual(show, { key: 'LFB', history: ['T', 'Blog'], format: 2 });
    assert.deepEqual(read(root, FILE).tasks, doc.tasks, 'numbers and references untouched');

    const bad = runAtm(['project', 'key', 'set', 'TaskMan'], root);
    assert.equal(bad.code, 2);
    assert.match(bad.stderr, /'TaskMan' isn't a project key: 3 to 6 letters and digits \(ATM\)/);
    const same = runAtm(['project', 'key', 'set', 'LFB'], root);
    assert.match(same.stderr, /the project's key is LFB already/);
  } finally {
    for (const root of roots) removeDir(root);
  }
});

test('commands take a key, an earlier prefix or a UUID, and show keys, provisional ones with a ~', () => {
  const root = makeProject({
    files: {
      [FILE]: {
        meta: { id: '3f2a9c1e', project: 'Agentic Task Manager' },
        tasks: [
          { id: 'T-001', name: 'one', type: 'feature', status: 'done' },
          { id: 'T-002', name: 'two', type: 'feature', status: 'todo', dependencies: ['T-001'] },
        ],
      },
    },
    git: true,
    commit: true,
  });
  const ok = (args) => {
    const r = runAtm(args, root);
    assert.equal(r.code, 0, `${args.join(' ')}: ${r.stderr}`);
    return r.stdout;
  };
  try {
    ok(['db', 'migrate', '--keys']);
    const two = read(root, FILE).tasks[1];
    // A new task: a UUID, the next number, provisional
    const dir = path.join(root, '..', `${path.basename(root)}-new`);
    fs.mkdirSync(dir);
    fs.writeFileSync(
      path.join(dir, 'new.json'),
      JSON.stringify([{ name: 'three', type: 'feature', dependencies: ['ATM-2'] }]),
    );
    ok(['task', 'add', '--json-file', path.join(dir, 'new.json')]);
    removeDir(dir);
    const three = read(root, FILE).tasks[2];
    assert.equal(three.number, 3);
    assert.equal(three.key_final, false);
    assert.deepEqual(three.dependencies, [two.id], 'stored by UUID');

    // The list shows keys, the provisional one marked
    const list = ok(['task', 'list']);
    assert.match(list, /^ATM-1 +done/m);
    assert.match(list, /^~ATM-3 +todo/m);
    const rows = JSON.parse(ok(['task', 'list', '--json']));
    assert.deepEqual(
      rows.map((r) => [r.key, r.key_final, r.label]),
      [
        ['ATM-1', true, 'ATM-1'],
        ['ATM-2', true, 'ATM-2'],
        ['ATM-3', false, '~ATM-3'],
      ],
    );
    assert.equal(rows[1].id, two.id, 'the UUID too');

    // Details by key, by the old prefix, by UUID: the same task, its dependency by key
    for (const ref of ['ATM-2', 'atm-2', 'T-002', two.id]) {
      const details = ok(['task', 'details', ref]);
      assert.match(details, /^ATM-2  two$/m, ref);
      assert.match(details, /depends on: ATM-1 \(done\)/, ref);
    }
    assert.equal(JSON.parse(ok(['task', 'details', 'ATM-2', '--json']))[0].key, 'ATM-2');

    // A refusal names tasks by key: ATM-3 waits for ATM-2
    const waiting = runAtm(['task', 'state', 'ATM-3', 'in_progress'], root);
    assert.match(waiting.stderr, /can't start ~ATM-3: waiting on ATM-2 \(todo\)/);
    // A state change and an edit by key
    ok(['task', 'state', 'ATM-2', 'in_progress']);
    assert.equal(read(root, FILE).tasks[1].status, 'in_progress');
    ok(['task', 'edit', 'T-002', '--add-related', 'ATM-1']);
    assert.deepEqual(read(root, FILE).tasks[1].related, [read(root, FILE).tasks[0].id]);

    // An unknown key says why
    const missing = runAtm(['task', 'details', 'ATM-99'], root);
    assert.notEqual(missing.code, 0);
    assert.match(missing.stderr, /no task 'ATM-99' in this project/);
    const other = runAtm(['task', 'details', 'LFB-1'], root);
    assert.match(other.stderr, /LFB isn't this project's key \(ATM\)/);
  } finally {
    removeDir(root);
  }
});

test('migrate --keys on a copy of this repository’s own task files loses nothing', () => {
  const own = JSON.parse(fs.readFileSync(path.join(REPO, FILE), 'utf8'));
  const ownArchive = JSON.parse(fs.readFileSync(path.join(REPO, ARCHIVE), 'utf8'));
  const root = makeProject({ files: { [FILE]: own, [ARCHIVE]: ownArchive } });
  try {
    const r = runAtm(['db', 'migrate', '--keys', '--json'], root);
    assert.equal(r.code, 0, r.stderr);
    const doc = read(root, FILE);
    const archived = read(root, ARCHIVE);
    const before = [...own.tasks, ...ownArchive.tasks];
    const after = [...doc.tasks, ...archived.tasks];
    assert.equal(after.length, before.length, 'every task, working and archived');
    assert.equal(doc.tasks.length, own.tasks.length);
    const uuidOf = new Map(before.map((t, i) => [t.id, after[i].id]));
    const numbers = new Set();
    const known = new Set(after.map((t) => t.id));
    after.forEach((t, i) => {
      const old = before[i];
      assert.match(t.id, UUID, old.id);
      assert.equal(t.key_final, true);
      assert.ok(!numbers.has(t.number), `${old.id}: number ${t.number} given once`);
      numbers.add(t.number);
      // T-527 keeps 527 (the file's prefix is T-)
      const own = /^T-(\d+)$/.exec(old.id);
      if (own) assert.equal(t.number, Number(own[1]), old.id);
      else assert.ok(t.aliases.includes(old.id), `${old.id} kept as an alias`);
      // Everything else as it was
      for (const [k, v] of Object.entries(old)) {
        if (['id', 'dependencies', 'related', 'duplicate_of'].includes(k)) continue;
        assert.deepEqual(t[k], v, `${old.id}.${k}`);
      }
      for (const field of ['dependencies', 'related']) {
        assert.deepEqual(
          t[field],
          old[field]?.map((d) => uuidOf.get(d) ?? d),
          `${old.id}.${field}: old IDs become their UUIDs`,
        );
        for (const d of t[field] ?? []) if (uuidOf.has(d) || UUID.test(d)) assert.ok(known.has(d), d);
      }
    });
    assert.equal(doc.meta.key, 'ATM', 'Agentic Task Manager');
    assert.deepEqual(doc.meta.key_history, ['T']);
    assert.ok(doc.meta.next_number > Math.max(...numbers));
    assert.equal(doc.meta.sprints.length, own.meta.sprints.length);
    // The file stays one atm reads
    const status = runAtm(['project', 'status', '--json'], root);
    assert.equal(status.code, 0, status.stderr);
    assert.equal(JSON.parse(status.stdout).tasks, own.tasks.length);
  } finally {
    removeDir(root);
  }
});
