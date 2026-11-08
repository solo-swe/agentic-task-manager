// The release machinery's scripts (packaging/, T-452): the Homebrew formula from a release's checksums, and the
// checks release.sh makes before it tags. The workflows that run them (.github/workflows/release.yml, homebrew.yml)
// run on a tag; these check what can be checked without one.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { describe, test } from 'node:test';
import { REPO, git, removeDir, tempDir } from './helpers.mjs';

const script = (name) => path.join(REPO, 'packaging', name);
const run = (name, args, cwd = REPO) => spawnSync('sh', [script(name), ...args], { cwd, encoding: 'utf8' });
const TARGETS = [
  'aarch64-apple-darwin',
  'x86_64-apple-darwin',
  'aarch64-unknown-linux-gnu',
  'x86_64-unknown-linux-gnu',
];

describe('packaging', () => {
  test('the Homebrew formula has each platform’s archive with its checksum', () => {
    const dir = tempDir('atm-formula-');
    try {
      const sums = path.join(dir, 'SHA256SUMS');
      const lines = TARGETS.map((t, i) => `${String(i).repeat(64)}  atm-1.2.3-${t}.tar.gz`);
      fs.writeFileSync(sums, [...lines, `${'f'.repeat(64)}  atm-1.2.3-x86_64-pc-windows-msvc.zip`].join('\n') + '\n');
      const r = run('homebrew-formula.sh', ['1.2.3', sums]);
      assert.equal(r.status, 0, r.stderr);
      assert.match(r.stdout, /^class Atm < Formula$/m);
      assert.match(r.stdout, /^ {2}version "1\.2\.3"$/m);
      TARGETS.forEach((t, i) => {
        const url = `https://github.com/solo-swe/agentic-task-manager/releases/download/v1.2.3/atm-1.2.3-${t}.tar.gz`;
        assert.ok(r.stdout.includes(`url "${url}"\n      sha256 "${String(i).repeat(64)}"`), t);
      });
      assert.match(r.stdout, /bin\.install "atm"/);
    } finally {
      removeDir(dir);
    }
  });

  test('a checksum missing from SHA256SUMS stops the formula', () => {
    const dir = tempDir('atm-formula-');
    try {
      const sums = path.join(dir, 'SHA256SUMS');
      fs.writeFileSync(sums, `${'a'.repeat(64)}  atm-1.2.3-aarch64-apple-darwin.tar.gz\n`);
      const r = run('homebrew-formula.sh', ['1.2.3', sums]);
      assert.notEqual(r.status, 0);
      assert.match(r.stderr, /atm-1\.2\.3-x86_64-apple-darwin\.tar\.gz isn't in /);
    } finally {
      removeDir(dir);
    }
  });

  // ADR-002: the public skills repo is shared; atm's release changes only atm's part of it
  test('the skills export replaces atm’s skills and README section, and leaves another app’s alone', () => {
    const dir = tempDir('atm-skills-repo-');
    try {
      git(['init', '-q'], dir);
      const other = path.join(dir, 'skills/other-thing');
      fs.mkdirSync(other, { recursive: true });
      fs.writeFileSync(path.join(other, 'SKILL.md'), 'another app’s skill\n');
      const gone = path.join(dir, 'skills/atm-gone');
      fs.mkdirSync(gone, { recursive: true });
      fs.writeFileSync(path.join(gone, 'SKILL.md'), 'a skill atm no longer has\n');
      fs.writeFileSync(
        path.join(dir, 'README.md'),
        '# Skills\n\n<!-- other:begin -->\n## other\n<!-- other:end -->\n\n<!-- atm:begin -->\n## atm\nold\n<!-- atm:end -->\n',
      );

      const r = run('skills-export.sh', ['1.2.3', dir]);
      assert.equal(r.status, 0, r.stderr);
      const ours = fs.readdirSync(path.join(REPO, 'skills')).filter((n) => n.startsWith('atm-'));
      for (const name of ours)
        assert.equal(
          fs.readFileSync(path.join(dir, 'skills', name, 'SKILL.md'), 'utf8'),
          fs.readFileSync(path.join(REPO, 'skills', name, 'SKILL.md'), 'utf8'),
          name,
        );
      assert.equal(fs.existsSync(gone), false, 'a skill atm dropped goes');
      assert.equal(fs.readFileSync(path.join(other, 'SKILL.md'), 'utf8'), 'another app’s skill\n');
      assert.ok(!fs.existsSync(path.join(dir, 'skills/claude-code.json')), 'Claude Code’s pre-approvals stay here');
      const readme = fs.readFileSync(path.join(dir, 'README.md'), 'utf8');
      assert.match(readme, /<!-- other:begin -->\n## other\n<!-- other:end -->/);
      assert.match(readme, /<!-- atm:begin -->\n## atm\n[\s\S]*version 1\.2\.3[\s\S]*<!-- atm:end -->/);
      assert.doesNotMatch(readme, /\nold\n/);

      // Again: the same files, one atm section
      assert.equal(run('skills-export.sh', ['1.2.3', dir]).status, 0);
      assert.equal(fs.readFileSync(path.join(dir, 'README.md'), 'utf8'), readme);
      assert.equal(run('skills-export.sh', ['not-a-version', dir]).status, 1);
    } finally {
      removeDir(dir);
    }
  });

  test('release.sh tags only the version Cargo.toml has, on a clean main that is pushed', () => {
    const origin = tempDir('atm-origin-');
    const root = tempDir('atm-release-');
    try {
      git(['init', '-q', '--bare', '-b', 'main'], origin);
      git(['init', '-q', '-b', 'main'], root);
      git(['config', 'user.email', 't@example.com'], root);
      git(['config', 'user.name', 'T'], root);
      fs.writeFileSync(path.join(root, 'Cargo.toml'), '[workspace.package]\nversion = "1.2.3"\n');
      git(['add', 'Cargo.toml'], root);
      git(['commit', '-qm', 'one'], root);
      git(['remote', 'add', 'origin', origin], root);
      git(['push', '-q', 'origin', 'main'], root);

      assert.match(run('release.sh', ['1.2.4'], root).stderr, /Cargo\.toml says 1\.2\.3, not 1\.2\.4/);
      assert.match(run('release.sh', ['latest'], root).stderr, /latest isn't a version/);
      fs.writeFileSync(path.join(root, 'notes.txt'), 'x');
      assert.match(run('release.sh', ['1.2.3'], root).stderr, /the working tree has changes/);
      fs.rmSync(path.join(root, 'notes.txt'));

      const r = run('release.sh', ['1.2.3'], root);
      assert.equal(r.status, 0, r.stderr);
      assert.match(r.stdout, /Push it to build the release: git push origin v1\.2\.3/);
      assert.equal(git(['tag', '--list'], root), 'v1.2.3');
      assert.match(run('release.sh', ['1.2.3'], root).stderr, /v1\.2\.3 exists already/);
    } finally {
      removeDir(origin);
      removeDir(root);
    }
  });
});
