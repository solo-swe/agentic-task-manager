// Task keys (ADR-007, T-540, T-543): `atm project key set` turns a task file of IDs (T-527) into one stored by UUID and
// shown by the project's key (ATM-527), and `atm project key set` changes the key. What a scenario can't say with one
// fixed output: the UUIDs are worked out (the same in every clone), and this repository's own file must come through
// whole.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { test } from 'node:test';
import { REPO, git, makeProject, removeDir, runAtm, tempDir } from './helpers.mjs';

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
    // Plain migrate leaves IDs alone; setting a key gives the tasks keys
    runAtm(['db', 'migrate'], root);
    assert.equal(read(root, FILE).tasks[0].id, 'T-007');
    const r = runAtm(['project', 'key', 'set', 'LFB', '--json'], root);
    assert.equal(r.code, 0, r.stderr);
    const keys = JSON.parse(r.stdout).migrated;
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
      assert.equal(t.key_final, undefined, 'plain keys (ADR-009)');
    }
    assert.deepEqual([seven.number, odd.number, one.number], [7, 9, 1]);
    assert.deepEqual(odd.aliases, ['T-P1-03']);
    assert.deepEqual(seven.dependencies, [one.id]);
    assert.deepEqual(seven.related, [odd.id]);

    // Another clone of the same file migrates to the same UUIDs, so their branches merge task by task
    assert.equal(runAtm(['project', 'key', 'set', 'LFB'], clone).code, 0);
    assert.deepEqual(read(clone, FILE).tasks, doc.tasks);

    // The key changes; the old ones are kept, in order, so their keys still find the tasks
    const set = runAtm(['project', 'key', 'set', 'Blog', '--json'], root);
    assert.equal(set.code, 0, set.stderr);
    assert.deepEqual(JSON.parse(set.stdout), {
      from: 'LFB',
      to: 'Blog',
      history: ['T', 'LFB'],
      migrated: null,
      also_used_by: [],
    });
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

test('commands take a key (with an old ~ too), an earlier prefix or a UUID, and show keys plain (ADR-009)', () => {
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
    ok(['project', 'key', 'set', 'ATM']);
    const two = read(root, FILE).tasks[1];
    // A new task: a UUID, the next number, its key plain from the start
    const dir = path.join(root, '..', `${path.basename(root)}-new`);
    fs.mkdirSync(dir);
    fs.writeFileSync(
      path.join(dir, 'new.json'),
      JSON.stringify([{ name: 'three', type: 'feature', dependencies: ['ATM-2'] }]),
    );
    assert.match(ok(['task', 'add', '--json-file', path.join(dir, 'new.json')]), /^Added ATM-3 three$/m);
    removeDir(dir);
    const three = read(root, FILE).tasks[2];
    assert.equal(three.number, 3);
    assert.equal(three.key_final, undefined);
    assert.deepEqual(three.dependencies, [two.id], 'stored by UUID');

    // The list shows keys, all plain
    const list = ok(['task', 'list']);
    assert.match(list, /^ATM-1 +done/m);
    assert.match(list, /^ATM-3 +todo/m);
    assert.doesNotMatch(list, /~/);
    const rows = JSON.parse(ok(['task', 'list', '--json']));
    assert.deepEqual(
      rows.map((r) => [r.key, r.key_final, r.label]),
      [
        ['ATM-1', undefined, 'ATM-1'],
        ['ATM-2', undefined, 'ATM-2'],
        ['ATM-3', undefined, 'ATM-3'],
      ],
    );
    // A key written with the old ~ still finds its task
    assert.match(ok(['task', 'details', '~ATM-3']), /^ATM-3  three$/m);
    assert.equal(rows[1].id, two.id, 'the UUID too');

    // Details by key, by the old prefix, by UUID: the same task, its dependency by key
    for (const ref of ['ATM-2', 'atm-2', 'T-002', two.id]) {
      const details = ok(['task', 'details', ref]);
      assert.match(details, /^ATM-2  two$/m, ref);
      assert.match(details, /depends on {2}ATM-1 \(done\)/, ref);
    }
    assert.equal(JSON.parse(ok(['task', 'details', 'ATM-2', '--json']))[0].key, 'ATM-2');

    // A refusal names tasks by key: ATM-3 waits for ATM-2
    const waiting = runAtm(['task', 'state', 'ATM-3', 'in_progress'], root);
    assert.match(waiting.stderr, /can't start ATM-3: waiting on ATM-2 \(todo\)/);
    // A state change and an edit by key
    assert.match(ok(['task', 'state', 'ATM-2', 'in_progress']), /^ATM-2: todo -> in_progress$/m);
    assert.match(ok(['task', 'note', 'ATM-2', 'a note']), /^ATM-2: note added \(1 in all\)$/m);
    assert.match(ok(['task', 'deps', 'ATM-3']), /^ATM-3 waits for: ATM-2 \(in_progress\)$/m);
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

test('every line for people names tasks by key, never by UUID; --json has the key too (ATM-557)', () => {
  const root = makeProject({
    files: {
      [FILE]: {
        meta: {
          id: '3f2a9c1e',
          project: 'Agentic Task Manager',
          components: [{ id: 'cli', label: 'CLI', paths: ['src/**'] }],
        },
        tasks: [
          { id: 'T-001', name: 'one', type: 'feature', status: 'done', files: ['docs/x.md'] },
          { id: 'T-002', name: 'two', type: 'feature', status: 'todo', files: ['docs/y.md'] },
          { id: 'T-003', name: 'three', type: 'feature', status: 'todo', dependencies: ['T-002'] },
        ],
      },
    },
    git: true,
    commit: true,
  });
  const ANY_UUID = /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/;
  const ok = (args) => {
    const r = runAtm(args, root);
    assert.ok(r.code === 0 || (args[0] === 'doctor' && r.code <= 1), `${args.join(' ')}: ${r.stderr}`);
    // JSON holds the UUID on purpose (`id`); a line for people never does
    if (!args.includes('--json'))
      assert.doesNotMatch(
        r.stdout + r.stderr,
        ANY_UUID,
        `${args.join(' ')} names a task by UUID:\n${r.stdout}${r.stderr}`,
      );
    return r.stdout;
  };
  try {
    ok(['project', 'key', 'set', 'ATM']);
    // The backlog's table, by key
    assert.match(ok(['sprint', 'backlog']), /^ATM-2 +todo/m);
    // Doctor's grouped warning: its tasks' keys
    assert.match(ok(['doctor']), /its files match no component.*\n {2}ATM-1 ATM-2/);
    // Moving into a sprint says which task moved, by key
    // Not "Next": a word atm takes for itself (ATM-665)
    ok(['sprint', 'add', 'Later']);
    assert.match(ok(['sprint', 'move', 'ATM-2', '--to', 'Later']), /^ATM-2: backlog -> Later$/m);
    ok(['sprint', 'start', 'Later']);
    for (const args of [
      ['sprint', 'list'],
      ['project', 'status'],
      ['project', 'status', '--all'],
      ['task', 'list', '--blocked'],
      ['task', 'deps', 'ATM-3'],
    ])
      ok(args);
    // The loop's and a state change's JSON carry the key beside the UUID
    const next = JSON.parse(ok(['loop', 'next', '--claim', '--json']));
    assert.equal(next.task.key, 'ATM-2');
    assert.match(next.task.id, ANY_UUID);
    ok(['claim', 'list']);
    const done = JSON.parse(runAtm(['task', 'state', 'ATM-2', 'done', '--json'], root).stdout);
    assert.equal(done.key, 'ATM-2');
    // Done says what it made ready, by key
    assert.match(ok(['task', 'state', 'ATM-3', 'in_progress']), /^ATM-3: todo -> in_progress$/m);
  } finally {
    removeDir(root);
  }
});

// ADR-009 (ATM-635): a key is plain from creation, on a branch as on main; an old file's key_final is dropped once by
// migrate; a direct push records nothing (`atm vcs pushed` without --pr stays, for skills installed before)
test('a key is plain from creation; migrate drops an old key_final once; a direct push changes no task', () => {
  const root = makeProject({
    files: { [FILE]: { meta: { id: '3f2a9c1e', project: 'Fig' }, tasks: [] } },
    git: true,
    commit: true,
  });
  const add = (name) => {
    const dir = tempDir();
    try {
      fs.writeFileSync(path.join(dir, 'new.json'), JSON.stringify([{ name, type: 'feature' }]));
      assert.equal(runAtm(['task', 'add', '--json-file', path.join(dir, 'new.json')], root).code, 0);
    } finally {
      removeDir(dir);
    }
  };
  try {
    assert.equal(runAtm(['project', 'key', 'set', 'FIG'], root).code, 0);
    git(['switch', '-q', '-c', 'work'], root);
    add('on a branch');
    assert.match(runAtm(['task', 'list'], root).stdout, /^FIG-1 /m, 'plain on a branch');
    // A file an earlier atm wrote, with key_final on its tasks: dropped once, and said
    const doc = read(root, FILE);
    doc.tasks[0].key_final = false;
    fs.writeFileSync(path.join(root, FILE), JSON.stringify(doc, null, 2));
    const r = runAtm(['db', 'migrate', '--json'], root);
    assert.equal(r.code, 0, r.stderr);
    assert.equal(JSON.parse(r.stdout).key_final_dropped, 1);
    assert.equal(read(root, FILE).tasks[0].key_final, undefined);
    assert.match(runAtm(['db', 'migrate'], root).stdout, /^Nothing to migrate\.$/m);
    // A direct push records nothing
    const before = fs.readFileSync(path.join(root, FILE), 'utf8');
    const pushed = runAtm(['vcs', 'pushed'], root);
    assert.equal(pushed.code, 0, pushed.stderr);
    assert.equal(pushed.stdout, 'Nothing to record without --pr: a direct push changes no task.\n');
    assert.deepEqual(JSON.parse(runAtm(['vcs', 'pushed', '--json'], root).stdout), { tasks: [] });
    assert.equal(fs.readFileSync(path.join(root, FILE), 'utf8'), before);
  } finally {
    removeDir(root);
  }
});

test('a dependency on another registered project’s task, by its key: waits until it is done there (T-544)', () => {
  const home = tempDir('atm-home-');
  const env = { ATM_HOME: home };
  const project = (id, name, key, tasks) => {
    const root = makeProject({ files: { [FILE]: { meta: { id, project: name }, tasks } } });
    assert.equal(runAtm(['db', 'migrate'], root, env).code, 0, 'registered');
    assert.equal(runAtm(['project', 'key', 'set', key], root, env).code, 0);
    return root;
  };
  const atm = project('aaaa1111', 'Agentic Task Manager', 'ATM', [
    { id: 'T-1', name: 'the API', type: 'feature', status: 'todo' },
  ]);
  const blog = project('bbbb2222', 'Learn Forever Blog', 'LFB', [
    { id: 'T-1', name: 'a post about it', type: 'feature', status: 'todo' },
  ]);
  const run = (root, args) => runAtm(args, root, env);
  try {
    const edit = run(blog, ['task', 'edit', 'LFB-1', '--add-dep', 'ATM-1']);
    assert.equal(edit.code, 0, edit.stderr);
    const apiUuid = read(atm, FILE).tasks[0].id;
    assert.deepEqual(read(blog, FILE).tasks[0].dependencies, [`aaaa1111:${apiUuid}`], 'the project’s ID and the UUID');
    // It waits for ATM-1, shown by its key
    assert.match(run(blog, ['task', 'details', 'LFB-1']).stdout, /depends on {2}ATM-1 \(todo\)/);
    assert.equal(run(blog, ['loop', 'next']).code, 3, 'nothing ready: LFB-1 waits for ATM-1');
    // Done there: ready here
    assert.equal(run(atm, ['task', 'state', 'ATM-1', 'in_progress']).code, 0);
    assert.equal(run(atm, ['task', 'state', 'ATM-1', 'done']).code, 0);
    assert.match(run(blog, ['loop', 'next']).stdout, /LFB-1 a post about it/);
    // The other project gone: it counts as not done, and doctor says why
    removeDir(atm);
    assert.equal(run(blog, ['loop', 'next']).code, 3);
    assert.match(run(blog, ['doctor']).stdout, /another project's task that can't be found/);
    // A key no registered project has
    const none = run(blog, ['task', 'edit', 'LFB-1', '--add-dep', 'FIG-1']);
    assert.notEqual(none.code, 0);
    assert.match(none.stderr, /FIG isn't this project's key/);
  } finally {
    removeDir(atm);
    removeDir(blog);
    removeDir(home);
  }
});

test('the task views and the summary show another project’s task by its key, status and project (T-549)', () => {
  const home = tempDir('atm-home-');
  const env = { ATM_HOME: home };
  const project = (id, name, key, tasks) => {
    const root = makeProject({ files: { [FILE]: { meta: { id, project: name }, tasks } } });
    assert.equal(runAtm(['db', 'migrate'], root, env).code, 0, 'registered');
    assert.equal(runAtm(['project', 'key', 'set', key], root, env).code, 0);
    return root;
  };
  const atm = project('aaaa1111', 'Agentic Task Manager', 'ATM', [
    { id: 'T-1', name: 'the API', type: 'feature', status: 'todo' },
  ]);
  const blog = project('bbbb2222', 'Learn Forever Blog', 'LFB', [
    { id: 'T-1', name: 'a post about it', type: 'feature', status: 'todo' },
  ]);
  const run = (root, args) => runAtm(args, root, env);
  const views = () => JSON.parse(run(blog, ['task', 'list', '--derived']).stdout);
  try {
    assert.equal(run(blog, ['task', 'edit', 'LFB-1', '--add-dep', 'ATM-1']).code, 0);
    const apiUuid = read(atm, FILE).tasks[0].id;
    const ref = `aaaa1111:${apiUuid}`;
    // A reference to a task that registered project doesn't have, and to a project nobody registered
    const gone = 'aaaa1111:0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8f99';
    const stranger = 'dddd4444:0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8f98';
    const data = read(blog, FILE);
    data.tasks[0].related = [gone, stranger];
    fs.writeFileSync(path.join(blog, FILE), JSON.stringify(data, null, 2));

    const foreign = views()[0].derived.foreign;
    assert.deepEqual(foreign[ref], {
      project: 'aaaa1111',
      project_name: 'Agentic Task Manager',
      places: [{ workspace: 'default', project: 'aaaa1111' }],
      found: true,
      name: 'the API',
      status: 'todo',
      archived: false,
      kind: 'todo',
      key: 'ATM-1',
      ambiguous: false,
    });
    assert.equal(foreign[gone].found, false);
    assert.equal(foreign[gone].reason, 'missing');
    assert.equal(foreign[gone].project_name, 'Agentic Task Manager');
    assert.deepEqual(foreign[stranger], { project: 'dddd4444', found: false, reason: 'unregistered' });
    // The summary's board names what LFB-1 waits for by its key there, and gives LFB-1's own
    const card = JSON.parse(run(blog, ['project', 'summary']).stdout).board.todo.cards[0];
    assert.equal(card.key, 'LFB-1');
    assert.equal(card.key_final, undefined);
    assert.deepEqual(card.waiting_for, [ref]);
    assert.deepEqual(card.waiting_for_labels, ['ATM-1']);
    // Done there: done here too
    assert.equal(run(atm, ['task', 'state', 'ATM-1', 'in_progress']).code, 0);
    assert.equal(run(atm, ['task', 'state', 'ATM-1', 'done']).code, 0);
    assert.equal(views()[0].derived.foreign[ref].kind, 'done');
    // The project gone: unregistered, as far as this project can tell
    removeDir(atm);
    assert.equal(views()[0].derived.foreign[ref].reason, 'unregistered');
  } finally {
    removeDir(atm);
    removeDir(blog);
    removeDir(home);
  }
});

test('two projects with one key: refused by key, named by UUID, shown by UUID, warned about (T-545)', () => {
  const home = tempDir('atm-home-');
  const env = { ATM_HOME: home };
  const project = (id, name, key) => {
    const root = makeProject({
      files: {
        [FILE]: {
          meta: { id, project: name },
          tasks: [{ id: 'T-1', name: `${name} one`, type: 'feature', status: 'todo' }],
        },
      },
    });
    assert.equal(runAtm(['db', 'migrate'], root, env).code, 0);
    assert.equal(runAtm(['project', 'key', 'set', key], root, env).code, 0);
    return root;
  };
  const first = project('aaaa1111', 'First', 'ATM');
  const second = project('cccc3333', 'Second', 'ATM');
  const blog = project('bbbb2222', 'Blog', 'LFB');
  const run = (root, args) => runAtm(args, root, env);
  try {
    // By key: refused, saying why
    const byKey = run(blog, ['task', 'edit', 'LFB-1', '--add-dep', 'ATM-1']);
    assert.notEqual(byKey.code, 0);
    assert.match(byKey.stderr, /2 projects use the key ATM: rename one with atm project key set/);
    // By UUID: found, stored with its project
    const uuid = read(second, FILE).tasks[0].id;
    const byUuid = run(blog, ['task', 'edit', 'LFB-1', '--add-dep', uuid]);
    assert.equal(byUuid.code, 0, byUuid.stderr);
    assert.deepEqual(read(blog, FILE).tasks[0].dependencies, [`cccc3333:${uuid}`]);
    // Shown by its UUID's first 8 characters, as ATM-1 would be ambiguous
    assert.match(
      run(blog, ['task', 'details', 'LFB-1']).stdout,
      new RegExp(`depends on {2}${uuid.slice(0, 8)} \\(todo\\)`),
    );
    // Inside each project its own keys stay, and doctor and status warn
    assert.match(run(first, ['task', 'list']).stdout, /^ATM-1 /m);
    assert.match(run(first, ['doctor']).stdout, /Two projects in your workspaces use the key ATM \(Second/);
    assert.match(run(first, ['project', 'status']).stdout, /warning: Two projects in your workspaces use the key ATM/);
    assert.doesNotMatch(run(blog, ['project', 'status']).stdout, /use the key/);
  } finally {
    for (const root of [first, second, blog]) removeDir(root);
    removeDir(home);
  }
});

// This repository's own files were keyed ATM on 2026-11-17 (`atm project key set ATM`), so a copy of them can't be keyed
// again: what is checked of them now is that they came through whole. A file still of IDs (another checkout) gets
// the migration check below.
test('this repository’s own task files, keyed: a UUID and one number each, known references, old IDs still found', () => {
  const own = JSON.parse(fs.readFileSync(path.join(REPO, FILE), 'utf8'));
  const ownArchive = JSON.parse(fs.readFileSync(path.join(REPO, ARCHIVE), 'utf8'));
  if (own.meta.format !== 2) return;
  const root = makeProject({ files: { [FILE]: own, [ARCHIVE]: ownArchive } });
  try {
    assert.equal(own.meta.key, 'ATM');
    assert.deepEqual(own.meta.key_history, ['T']);
    const all = [...own.tasks, ...ownArchive.tasks];
    const known = new Set(all.map((t) => t.id));
    const numbers = new Set();
    for (const t of all) {
      assert.match(t.id, UUID, t.name);
      assert.ok(Number.isInteger(t.number) && !numbers.has(t.number), `${t.name}: number ${t.number} once`);
      numbers.add(t.number);
      for (const d of [...(t.dependencies ?? []), ...(t.related ?? [])])
        if (!d.includes(':')) assert.ok(known.has(d), `${t.name}: ${d} is a task of the project`);
    }
    assert.ok(own.meta.next_number > Math.max(...numbers));
    // The old IDs still find their tasks, an archived one too (T-413 is archived later, T-556 is in the file)
    for (const [old, key] of [
      ['T-556', 'ATM-556'],
      ['T-1', 'ATM-1'],
    ]) {
      const r = runAtm(['task', 'details', old, '--json'], root);
      assert.equal(r.code, 0, `${old}: ${r.stderr}`);
      assert.equal(JSON.parse(r.stdout)[0].key, key, old);
    }
    const doctor = JSON.parse(runAtm(['doctor', '--json'], root).stdout);
    assert.deepEqual(doctor.errors, []);
  } finally {
    removeDir(root);
  }
});

test('migrate --keys on a copy of this repository’s own task files loses nothing (while they are of IDs)', () => {
  const own = JSON.parse(fs.readFileSync(path.join(REPO, FILE), 'utf8'));
  const ownArchive = JSON.parse(fs.readFileSync(path.join(REPO, ARCHIVE), 'utf8'));
  if (own.meta.format === 2) return;
  const root = makeProject({ files: { [FILE]: own, [ARCHIVE]: ownArchive } });
  try {
    const r = runAtm(['project', 'key', 'set', 'ATM', '--json'], root);
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
      assert.equal(t.key_final, undefined);
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
