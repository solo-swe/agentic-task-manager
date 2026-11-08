// `atm agents install|status|uninstall` (T-442): atm's skills, this version's, where Claude Code reads them. Every
// test runs with its own HOME and ATM_HOME, never the machine's ~/.claude.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { describe, test } from 'node:test';
import { REPO, makeProject, removeDir, runAtm, tempDir } from './helpers.mjs';

const SKILLS = fs
  .readdirSync(path.join(REPO, 'skills'), { withFileTypes: true })
  .filter((d) => d.isDirectory())
  .map((d) => d.name)
  .sort();
const ALLOWED = JSON.parse(fs.readFileSync(path.join(REPO, 'skills/claude-code.json'), 'utf8'))['allowed-tools'];

/** A home folder and an atm folder of the test's own; `agents(args)` runs `atm agents …` with them */
function setup() {
  const home = tempDir('atm-agents-home-');
  const atmHome = path.join(home, '.agentic-task-manager');
  const cwd = makeProject({ files: { '.agentic-task-manager/tasks.json': { meta: { project: 'P' }, tasks: [] } } });
  const agents = (args, where = cwd) => runAtm(['agents', ...args], where, { HOME: home, ATM_HOME: atmHome });
  const skills = path.join(home, '.claude/skills');
  return { home, atmHome, cwd, agents, skills, done: () => [home, cwd].forEach(removeDir) };
}

const skillText = (folder) => fs.readFileSync(path.join(folder, 'SKILL.md'), 'utf8');

describe('atm agents', () => {
  test('install links the skills into ~/.claude/skills, with Claude Code’s pre-approvals; again, nothing changes', () => {
    const t = setup();
    try {
      // Before: Claude Code not found, every skill missing
      const before = JSON.parse(t.agents(['status', '--json']).stdout);
      assert.deepEqual([before[0].harness, before[0].detected], ['claude', false]);
      assert.ok(before[0].skills.every((s) => s.state === 'missing'));

      const r = t.agents(['install']);
      assert.equal(r.code, 0, r.stderr);
      for (const name of SKILLS) {
        const link = path.join(t.skills, name);
        assert.ok(fs.lstatSync(link).isSymbolicLink(), name);
        assert.equal(fs.realpathSync(link), fs.realpathSync(path.join(t.atmHome, 'skills/claude', name)));
        const text = skillText(link);
        assert.match(text, new RegExp(`^---\\nname: ${name}\\n`));
        assert.ok(text.includes(`\nallowed-tools: ${ALLOWED[name]}\n---\n`), name);
        assert.match(r.stdout, new RegExp(`installed: ${link} \\(link\\)`));
      }
      assert.match(r.stdout, new RegExp(`${SKILLS.length} written, 0 unchanged`));

      const again = t.agents(['install']);
      assert.equal(again.code, 0, again.stderr);
      assert.match(again.stdout, new RegExp(`0 written, ${SKILLS.length} unchanged`));
      const status = JSON.parse(t.agents(['status', '--json']).stdout)[0];
      assert.equal(status.detected, true);
      assert.deepEqual(new Set(status.skills.map((s) => s.state)), new Set(['linked']));
    } finally {
      t.done();
    }
  });

  test('--copy copies; a changed copy is stale, and install puts it right', () => {
    const t = setup();
    try {
      assert.equal(t.agents(['install', '--copy']).code, 0);
      const one = path.join(t.skills, SKILLS[0]);
      assert.ok(!fs.lstatSync(one).isSymbolicLink());
      fs.writeFileSync(path.join(one, 'SKILL.md'), 'edited by hand\n');
      const stale = JSON.parse(t.agents(['status', '--json']).stdout)[0].skills;
      assert.equal(stale.find((s) => s.skill === SKILLS[0]).state, 'stale');
      assert.equal(stale.find((s) => s.skill === SKILLS[1]).state, 'copied');
      const fix = t.agents(['install', '--copy']);
      assert.match(fix.stdout, new RegExp(`updated: ${one} \\(copy\\)`));
      assert.match(fix.stdout, /1 written, 6 unchanged/);
    } finally {
      t.done();
    }
  });

  test('a folder atm didn’t write is left alone (exit 1, saying so); uninstall removes only what atm wrote', () => {
    const t = setup();
    try {
      const mine = path.join(t.skills, 'atm-push');
      fs.mkdirSync(mine, { recursive: true });
      fs.writeFileSync(path.join(mine, 'SKILL.md'), 'my own\n');
      fs.writeFileSync(path.join(t.skills, 'notes.txt'), 'mine too\n');

      const r = t.agents(['install']);
      assert.equal(r.code, 1);
      assert.match(r.stderr, new RegExp(`left alone: ${mine} is there already, and atm didn't put it there`));
      assert.equal(skillText(mine), 'my own\n');
      assert.ok(fs.lstatSync(path.join(t.skills, 'atm-do-task')).isSymbolicLink());
      assert.equal(
        JSON.parse(t.agents(['status', '--json']).stdout)[0].skills.find((s) => s.skill === 'atm-push').state,
        "not atm's",
      );

      const u = t.agents(['uninstall']);
      assert.equal(u.code, 0, u.stderr);
      assert.equal(u.stdout.split('\n').filter((l) => l.startsWith('removed: ')).length, SKILLS.length - 1);
      assert.deepEqual(fs.readdirSync(t.skills).sort(), ['atm-push', 'notes.txt']);
      assert.equal(skillText(mine), 'my own\n');
      assert.ok(!fs.existsSync(path.join(t.atmHome, 'skills/claude')));
      assert.equal(t.agents(['uninstall']).stdout, 'nothing atm installed to remove\n');
    } finally {
      t.done();
    }
  });

  test('--project copies into the project’s .claude/skills (to commit), and uninstall --project removes only those', () => {
    const t = setup();
    try {
      assert.equal(t.agents(['install']).code, 0);
      const r = t.agents(['install', '--project'], path.join(t.cwd, '.agentic-task-manager'));
      assert.equal(r.code, 0, r.stderr);
      const copy = path.join(t.cwd, '.claude/skills/atm-do-task');
      assert.ok(fs.statSync(copy).isDirectory() && !fs.lstatSync(copy).isSymbolicLink());
      assert.match(r.stdout, new RegExp(`installed: ${copy} \\(copy\\)`));
      const status = JSON.parse(t.agents(['status', '--project', '--json']).stdout);
      assert.deepEqual(
        status.map((h) => h.scope),
        ['user', 'project'],
      );
      assert.ok(status[1].skills.every((s) => s.state === 'copied'));

      assert.equal(t.agents(['uninstall', '--project']).code, 0);
      assert.deepEqual(fs.readdirSync(path.join(t.cwd, '.claude/skills')), []);
      assert.ok(fs.lstatSync(path.join(t.skills, 'atm-do-task')).isSymbolicLink(), 'the user’s are kept');
    } finally {
      t.done();
    }
  });

  test('an unknown harness is refused, naming the ones atm knows', () => {
    const t = setup();
    try {
      const r = t.agents(['install', '--harness', 'nope']);
      assert.notEqual(r.code, 0);
      assert.match(r.stderr, /no harness 'nope' \(atm knows: claude\)/);
      assert.ok(!fs.existsSync(t.skills));
    } finally {
      t.done();
    }
  });
});
