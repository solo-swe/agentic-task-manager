// `--dry-run` (`-n`): every command that writes says what it would do, and writes nothing (docs/agent/commands.md:
// "Every command takes --json and --dry-run"). One project with something for each command to act on; after each dry
// run, its files and atm's state folder are as they were, byte for byte.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { after, before, describe, test } from 'node:test';
import { makeProject, removeDir, runAtm } from './helpers.mjs';

/** Every file under `dir` and its text (git's own files left out but its config, where the merge driver goes). */
function snapshot(dir, base = dir, out = {}) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    const rel = path.relative(base, full);
    if (entry.isDirectory()) {
      if (rel === '.git') out['.git/config'] = fs.readFileSync(path.join(full, 'config'), 'utf8');
      else snapshot(full, base, out);
    } else out[rel] = fs.readFileSync(full, 'utf8');
  }
  return out;
}

// [args, what it says it would do]
const CASES = [
  // task
  [['task', 'add', '--name', 'New', '--type', 'feature'], '[dry-run] would add task New\n'],
  [['task', 'add', '--json-file', 'many.json'], '[dry-run] would add tasks from many.json\n'],
  [['task', 'edit', 'T-1', '--name', 'Renamed'], '[dry-run] would edit task T-1\n'],
  [['task', 'state', 'T-1', 'in_progress'], '[dry-run] would start T-1\n'],
  [['task', 'state', 'T-1', 'skipped', '--reason', 'x'], '[dry-run] would skip T-1\n'],
  [['task', 'note', 'T-1', 'hello'], '[dry-run] would add a note to T-1\n'],
  [['task', 'review', 'approve', 'T-3'], '[dry-run] would approve T-3\n'],
  // loop, claim
  [['loop', 'next'], '[dry-run] would show the next task\n'],
  [['loop', 'next', '--claim'], '[dry-run] would claim the next task\n'],
  [['claim', 'heartbeat', 'T-2'], '[dry-run] would extend the lease on T-2\n'],
  [['claim', 'heartbeat', '--auto'], '[dry-run] would renew claims due for it\n'],
  [['claim', 'hold', 'T-2'], '[dry-run] would hold the claim on T-2 while this shell runs\n'],
  // vcs
  [
    ['vcs', 'commit', 'T-2', '--file', 'a.txt', '--message', 'Do it'],
    '[dry-run] would run: atm vcs commit T-2 --file a.txt --message Do it\n',
  ],
  [
    ['vcs', 'pr', 'T-1', 'T-2', '--number', '8', '--url', 'https://e.com/8'],
    '[dry-run] would run: atm vcs pr T-1 T-2 --number 8 --url https://e.com/8\n',
  ],
  [
    ['vcs', 'pushed', '--pr', '7', '--at', '2026-05-01T00:00:00Z'],
    '[dry-run] would run: atm vcs pushed --pr 7 --at 2026-05-01T00:00:00Z\n',
  ],
  [['vcs', 'pushed'], '[dry-run] would run: atm vcs pushed\n'],
  [['vcs', 'repository', 'https://github.com/o/r'], '[dry-run] would run: atm vcs repository https://github.com/o/r\n'],
  // sprint
  [['sprint', 'add', 'S3'], '[dry-run] would add the sprint S3\n'],
  [['sprint', 'edit', 'S2', '--goal', 'More'], '[dry-run] would edit the sprint S2\n'],
  [['sprint', 'rename', 'S2', 'Later'], '[dry-run] would rename the sprint S2 to Later\n'],
  [['sprint', 'start', 'S2'], '[dry-run] would start the sprint S2\n'],
  [['sprint', 'close', 'S1'], '[dry-run] would close the sprint S1\n'],
  [['sprint', 'delete', 'S2'], '[dry-run] would delete the sprint S2\n'],
  [['sprint', 'move', 'T-1', '--to', 'S2'], '[dry-run] would move T-1 to S2\n'],
  // release, component, label
  [
    ['release', 'add', '2.0', '--name', 'Two', '--target', '2026-12-01'],
    '[dry-run] would add release 2.0 --name Two --target 2026-12-01\n',
  ],
  [['release', 'edit', '1.0', '--name', 'One'], '[dry-run] would edit release 1.0 --name One\n'],
  [['release', 'edit', '1.0', '--released'], '[dry-run] would edit release 1.0 --released (today)\n'],
  [['release', 'rename', '1.0', '1.0.0'], '[dry-run] would rename release 1.0 to 1.0.0\n'],
  [['release', 'archive', '1.0'], '[dry-run] would archive release 1.0\n'],
  [
    ['component', 'add', 'api', '--path', 'api/**', '--assign'],
    '[dry-run] would add component api --path api/** --assign\n',
  ],
  [['component', 'edit', 'ui', '--label', 'UI'], '[dry-run] would edit component ui --label UI\n'],
  [['component', 'rename', 'ui', 'web'], '[dry-run] would rename component ui to web\n'],
  [['label', 'rename', 'x', 'y'], '[dry-run] would rename the label x to y\n'],
  // project
  [['project', 'workflow', 'set', 'direct'], '[dry-run] would set the workflow to direct\n'],
  [['project', 'logo', 'set', 'logo.svg'], '[dry-run] would set the logo\n'],
  [['project', 'logo', 'unset'], '[dry-run] would unset the logo\n'],
  [
    ['project', 'vocab', 'rename', 'type', 'feature', 'feat', '--label', 'Feature'],
    '[dry-run] would rename the type feature to feat (label Feature)\n',
  ],
  [['project', 'key', 'set', 'DRY'], "[dry-run] would set the project's key to DRY\n"],
  // db
  [['db', 'unarchive', 'T-5'], '[dry-run] would unarchive T-5\n'],
  [['db', 'init'], '[dry-run] would give tasks.json a meta.id and register the merge driver\n'],
  // workspace
  [['workspace', 'create', 'work'], '[dry-run] would make the workspace work\n'],
  [['workspace', 'rename', 'old', 'new'], '[dry-run] would rename the workspace old to new\n'],
  [['workspace', 'delete', 'old'], '[dry-run] would delete the workspace old\n'],
  [['workspace', 'add-project', 'old'], '[dry-run] would add this folder to the workspace old\n'],
  [['workspace', 'remove-project', 'old', '.'], '[dry-run] would remove . from the workspace old\n'],
  [['workspace', 'prune', 'old'], '[dry-run] would remove the projects whose folder is gone from the workspace old\n'],
  // service
  [['service', 'start'], '[dry-run] would start the dashboard server\n'],
  [['service', 'stop'], '[dry-run] would stop the dashboard server\n'],
  [['service', '--refresh'], '[dry-run] would refresh the dashboard server if outdated\n'],
];

describe('--dry-run', () => {
  let root;
  let env;
  before(() => {
    root = makeProject({
      commit: true,
      files: {
        '.agentic-task-manager/tasks.json': {
          meta: {
            project: 'Dry',
            sprints: [
              { id: 'S1', state: 'active' },
              { id: 'S2', state: 'planned' },
            ],
            releases: [{ id: '1.0' }],
            components: [{ id: 'ui', paths: ['ui/**'] }],
            archived: 1,
          },
          tasks: [
            {
              id: 'T-1',
              name: 'One',
              type: 'feature',
              status: 'todo',
              sprint: 'S1',
              labels: ['x'],
            },
            {
              id: 'T-2',
              name: 'Two',
              type: 'feature',
              status: 'in_progress',
              started_by: 'bob',
            },
            { id: 'T-3', name: 'Three', type: 'feature', status: 'in_review' },
            {
              id: 'T-4',
              name: 'Four',
              type: 'feature',
              status: 'done',
              pr: { number: 7, url: 'u7' },
            },
          ],
        },
        '.agentic-task-manager/tasks-archive.json': {
          tasks: [{ id: 'T-5', name: 'Five', status: 'done' }],
        },
        'many.json': [{ name: 'A', type: 'feature' }],
        'logo.svg': '<svg xmlns="http://www.w3.org/2000/svg"/>',
        'a.txt': 'a\n',
      },
    });
    env = { ATM_HOME: path.join(root, '.atm-home'), ATM_AGENT: 'bob' };
    // A workspace to act on, and the project registered as running atm here does: what a dry run must leave alone
    assert.equal(runAtm(['workspace', 'create', 'old'], root, env).code, 0);
    assert.equal(runAtm(['workspace', 'add-project', 'old'], root, env).code, 0);
    assert.equal(runAtm(['project', 'status'], root, env).code, 0);
  });
  after(() => removeDir(root));

  // (-n is the same option: main.rs's tests)
  for (const [args, said] of CASES) {
    test(`atm ${args.join(' ')} --dry-run`, () => {
      const before = snapshot(root);
      const r = runAtm([...args, '--dry-run'], root, env);
      assert.equal(r.code, 0, r.stderr);
      assert.equal(r.stdout, said);
      assert.deepEqual(snapshot(root), before);
    });
  }
});
