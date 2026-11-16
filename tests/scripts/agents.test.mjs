// `atm agents install|status|uninstall` (T-442, T-413): atm's skills, this version's, where the harnesses read them:
// Claude Code's ~/.claude/skills, the shared ~/.agents/skills (Codex, Copilot, Cursor, OpenCode, Gemini CLI) and
// Antigravity's ~/.gemini/config/skills. Every test runs with its own HOME and ATM_HOME, never the machine's.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { describe, test } from 'node:test';
import { REPO, makeProject, removeDir, runAtm, shown, tempDir } from './helpers.mjs';

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
  const shared = path.join(home, '.agents/skills');
  return { home, atmHome, cwd, agents, skills, shared, done: () => [home, cwd].forEach(removeDir) };
}

const skillText = (folder) => fs.readFileSync(path.join(folder, 'SKILL.md'), 'utf8');
const statusOf = (t, args = []) => JSON.parse(t.agents(['status', ...args, '--json']).stdout);
const harness = (report, id, scope = 'user') => report.find((h) => h.harness === id && h.scope === scope);

describe('atm agents', () => {
  test('install links the skills into ~/.claude/skills (with Claude Code’s pre-approvals) and ~/.agents/skills (without); again, nothing changes', () => {
    const t = setup();
    try {
      // Before: no tool found, every skill missing
      const before = statusOf(t);
      assert.deepEqual(
        before.map((h) => [h.harness, h.detected]),
        [
          ['claude', false],
          ['agents', false],
          ['antigravity', false],
        ],
      );
      assert.ok(before.every((h) => h.skills.every((s) => s.state === 'missing')));
      assert.deepEqual(
        harness(before, 'agents').tools.map((tool) => tool.tool),
        ['codex', 'copilot', 'cursor', 'opencode', 'gemini'],
      );

      const r = t.agents(['install']);
      assert.equal(r.code, 0, r.stderr);
      for (const name of SKILLS) {
        const link = path.join(t.skills, name);
        assert.ok(fs.lstatSync(link).isSymbolicLink(), name);
        assert.equal(fs.realpathSync(link), fs.realpathSync(path.join(t.atmHome, 'skills/claude', name)));
        const text = skillText(link);
        assert.match(text, new RegExp(`^---\\nname: ${name}\\n`));
        assert.ok(text.includes(`\nallowed-tools: ${ALLOWED[name]}\n---\n`), name);
        assert.match(r.stdout, new RegExp(`installed: ${shown(link)} \\(link\\)`));

        const portable = path.join(t.shared, name);
        assert.ok(fs.lstatSync(portable).isSymbolicLink(), name);
        assert.equal(fs.realpathSync(portable), fs.realpathSync(path.join(t.atmHome, 'skills/agents', name)));
        assert.equal(skillText(portable), fs.readFileSync(path.join(REPO, 'skills', name, 'SKILL.md'), 'utf8'));
        assert.match(r.stdout, new RegExp(`installed: ${shown(portable)} \\(link\\)`));
      }
      // Antigravity isn't here, so its folder isn't made
      assert.ok(!fs.existsSync(path.join(t.home, '.gemini')));
      assert.match(r.stdout, new RegExp(`${2 * SKILLS.length} written, 0 unchanged`));

      const again = t.agents(['install']);
      assert.equal(again.code, 0, again.stderr);
      assert.match(again.stdout, new RegExp(`0 written, ${2 * SKILLS.length} unchanged`));
      const status = statusOf(t);
      assert.equal(harness(status, 'claude').detected, true);
      for (const id of ['claude', 'agents'])
        assert.deepEqual(new Set(harness(status, id).skills.map((s) => s.state)), new Set(['linked']), id);
      assert.ok(harness(status, 'antigravity').skills.every((s) => s.state === 'missing'));
    } finally {
      t.done();
    }
  });

  test('--harness takes a tool: codex installs into ~/.agents/skills only, and uninstall --harness cursor removes them (the same folder)', () => {
    const t = setup();
    try {
      const r = t.agents(['install', '--harness', 'codex']);
      assert.equal(r.code, 0, r.stderr);
      assert.match(r.stdout, new RegExp(`${SKILLS.length} written, 0 unchanged`));
      assert.ok(fs.lstatSync(path.join(t.shared, 'atm-do-task')).isSymbolicLink());
      assert.ok(!fs.existsSync(t.skills));

      const u = t.agents(['uninstall', '--harness', 'cursor']);
      assert.equal(u.code, 0, u.stderr);
      assert.equal(u.stdout.split('\n').filter((l) => l.startsWith('removed: ')).length, SKILLS.length);
      assert.deepEqual(fs.readdirSync(t.shared), []);
    } finally {
      t.done();
    }
  });

  test('a tool found here: Antigravity’s folder is installed by default; Cursor, reading both folders, is said to list them twice', () => {
    const t = setup();
    try {
      fs.mkdirSync(path.join(t.home, '.gemini/antigravity-cli'), { recursive: true });
      fs.mkdirSync(path.join(t.home, '.cursor'), { recursive: true });
      fs.mkdirSync(path.join(t.home, '.codex'), { recursive: true });

      const r = t.agents(['install']);
      assert.equal(r.code, 0, r.stderr);
      assert.match(r.stdout, new RegExp(`${3 * SKILLS.length} written, 0 unchanged`));
      const antigravity = path.join(t.home, '.gemini/config/skills/atm-do-task');
      assert.ok(fs.lstatSync(antigravity).isSymbolicLink());

      const status = statusOf(t);
      assert.deepEqual(harness(status, 'agents').duplicates, ['Cursor']);
      assert.deepEqual(harness(status, 'claude').duplicates, []);
      assert.deepEqual(
        harness(status, 'agents')
          .tools.filter((tool) => tool.detected)
          .map((tool) => tool.tool),
        ['codex', 'cursor'],
      );
      const text = t.agents(['status']).stdout;
      assert.match(text, /read by Codex, Cursor here/);
      assert.match(text, /listed twice in Cursor/);

      // Installed for Codex only, Cursor reads it once: nothing twice
      assert.equal(t.agents(['uninstall', '--harness', 'claude']).code, 0);
      assert.deepEqual(harness(statusOf(t), 'agents').duplicates, []);
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
      const stale = harness(statusOf(t), 'claude').skills;
      assert.equal(stale.find((s) => s.skill === SKILLS[0]).state, 'stale');
      assert.equal(stale.find((s) => s.skill === SKILLS[1]).state, 'copied');
      assert.ok(!fs.lstatSync(path.join(t.shared, SKILLS[0])).isSymbolicLink());
      const fix = t.agents(['install', '--copy']);
      assert.match(fix.stdout, new RegExp(`updated: ${shown(one)} \\(copy\\)`));
      assert.match(fix.stdout, new RegExp(`1 written, ${2 * SKILLS.length - 1} unchanged`));
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
      assert.match(r.stderr, new RegExp(`left alone: ${shown(mine)} is there already, and atm didn't put it there`));
      assert.equal(skillText(mine), 'my own\n');
      assert.ok(fs.lstatSync(path.join(t.skills, 'atm-do-task')).isSymbolicLink());
      assert.equal(harness(statusOf(t), 'claude').skills.find((s) => s.skill === 'atm-push').state, "not atm's");

      const u = t.agents(['uninstall']);
      assert.equal(u.code, 0, u.stderr);
      assert.equal(u.stdout.split('\n').filter((l) => l.startsWith('removed: ')).length, 2 * SKILLS.length - 1);
      assert.deepEqual(fs.readdirSync(t.skills).sort(), ['atm-push', 'notes.txt']);
      assert.deepEqual(fs.readdirSync(t.shared), []);
      assert.equal(skillText(mine), 'my own\n');
      assert.ok(!fs.existsSync(path.join(t.atmHome, 'skills/claude')));
      assert.ok(!fs.existsSync(path.join(t.atmHome, 'skills/agents')));
      assert.equal(t.agents(['uninstall']).stdout, 'nothing atm installed to remove\n');
    } finally {
      t.done();
    }
  });

  test('--project copies into the project’s .claude/skills and .agents/skills (to commit), and uninstall --project removes only those', () => {
    const t = setup();
    try {
      assert.equal(t.agents(['install']).code, 0);
      const r = t.agents(['install', '--project'], path.join(t.cwd, '.agentic-task-manager'));
      assert.equal(r.code, 0, r.stderr);
      for (const folder of ['.claude/skills', '.agents/skills']) {
        const copy = path.join(t.cwd, folder, 'atm-do-task');
        assert.ok(fs.statSync(copy).isDirectory() && !fs.lstatSync(copy).isSymbolicLink());
        assert.match(r.stdout, new RegExp(`installed: ${shown(copy)} \\(copy\\)`));
      }
      // Antigravity reads a project's .agents/skills: nothing of its own
      assert.match(r.stdout, new RegExp(`${2 * SKILLS.length} written, 0 unchanged`));
      const status = statusOf(t, ['--project']);
      assert.deepEqual(
        status.map((h) => [h.harness, h.scope]),
        [
          ['claude', 'user'],
          ['claude', 'project'],
          ['agents', 'user'],
          ['agents', 'project'],
          ['antigravity', 'user'],
        ],
      );
      assert.ok(harness(status, 'agents', 'project').skills.every((s) => s.state === 'copied'));

      assert.equal(t.agents(['uninstall', '--project']).code, 0);
      assert.deepEqual(fs.readdirSync(path.join(t.cwd, '.claude/skills')), []);
      assert.deepEqual(fs.readdirSync(path.join(t.cwd, '.agents/skills')), []);
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
      assert.match(
        r.stderr,
        /no harness 'nope' \(atm knows: claude, agents, antigravity, codex, copilot, cursor, opencode, gemini\)/,
      );
      assert.ok(!fs.existsSync(t.skills));
      assert.ok(!fs.existsSync(t.shared));
    } finally {
      t.done();
    }
  });
});
