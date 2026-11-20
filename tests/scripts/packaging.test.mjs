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

  // T-460: the app's universal DMG, its own atm on PATH, one or the other with the formula
  test('the Homebrew cask has the universal DMG with its checksum, puts the app’s atm on PATH and says to remove the formula', () => {
    const dir = tempDir('atm-cask-');
    try {
      const sums = path.join(dir, 'SHA256SUMS');
      fs.writeFileSync(
        sums,
        `${'a'.repeat(64)}  atm-1.2.3-aarch64-apple-darwin.tar.gz\n${'b'.repeat(64)}  atm-app-1.2.3-universal.dmg\n`,
      );
      const r = run('homebrew-cask.sh', ['1.2.3', sums]);
      assert.equal(r.status, 0, r.stderr);
      assert.match(r.stdout, /^cask "atm-app" do$/m);
      assert.match(r.stdout, /^ {2}version "1\.2\.3"$/m);
      assert.match(r.stdout, new RegExp(`^ {2}sha256 "${'b'.repeat(64)}"$`, 'm'));
      assert.ok(
        r.stdout.includes(
          'url "https://github.com/solo-swe/agentic-task-manager/releases/download/v#{version}/atm-app-#{version}-universal.dmg"',
        ),
      );
      // The bundle is named for the app (productName, T-537): the Dock and ⌘-Tab show its file's name
      assert.match(r.stdout, /^ {2}name "Agentic Task Manager"$/m);
      assert.match(r.stdout, /^ {2}app "Agentic Task Manager\.app"$/m);
      assert.ok(r.stdout.includes('binary "#{appdir}/Agentic Task Manager.app/Contents/MacOS/atm"'));
      // A cask declares conflicts only with casks (Homebrew refuses `conflicts_with formula:`), and `depends_on macos:`
      // only for a minimum newer than Homebrew's own: the caveats say to remove the formula
      assert.doesNotMatch(r.stdout, /conflicts_with/);
      // The app updates itself (T-483), so brew leaves an installed one alone
      assert.match(r.stdout, /^ {2}auto_updates true$/m);
      assert.match(r.stdout, /^ {2}depends_on :macos$/m);
      assert.ok(r.stdout.includes('`brew uninstall solo-swe/tap/atm`'));
      // It says how to connect agents, and does nothing to them
      assert.ok(r.stdout.includes('Run `atm agents install` to let your AI agents use atm.'));
      assert.doesNotMatch(r.stdout, /postflight|system_command/);

      fs.writeFileSync(sums, `${'a'.repeat(64)}  atm-1.2.3-aarch64-apple-darwin.tar.gz\n`);
      const missing = run('homebrew-cask.sh', ['1.2.3', sums]);
      assert.notEqual(missing.status, 0);
      assert.match(missing.stderr, /atm-app-1\.2\.3-universal\.dmg isn't in /);
    } finally {
      removeDir(dir);
    }
  });

  // T-483: the app's update manifest names the release's signed archive, for both Mac architectures, with the
  // version's What's new as its notes
  test('the update manifest names the signed universal archive and carries the version’s What’s new', () => {
    const dir = tempDir('atm-manifest-');
    const manifest = (args) =>
      spawnSync('node', [script('updater-manifest.mjs'), ...args], { cwd: REPO, encoding: 'utf8' });
    try {
      const sig = path.join(dir, 'Agentic Task Manager.app.tar.gz.sig');
      fs.writeFileSync(sig, 'dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZQ==\n');
      const notes = path.join(dir, 'WHATS-NEW.md');
      fs.writeFileSync(
        notes,
        '# What’s new\n\n## [1.2.3] - 2026-01-02\n\n- One thing.\n- Another "quoted" thing.\n\n## [1.2.2] - 2026-01-01\n\n- Older.\n',
      );
      const archive = 'atm-app-1.2.3-universal.app.tar.gz';
      const r = manifest(['1.2.3', archive, sig, notes]);
      assert.equal(r.status, 0, r.stderr);
      const json = JSON.parse(r.stdout);
      assert.equal(json.version, '1.2.3');
      assert.equal(json.notes, '- One thing.\n- Another "quoted" thing.');
      assert.match(json.pub_date, /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/);
      const expected = {
        signature: 'dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZQ==',
        url: `https://github.com/solo-swe/agentic-task-manager/releases/download/v1.2.3/${archive}`,
      };
      assert.deepEqual(json.platforms, { 'darwin-aarch64': expected, 'darwin-x86_64': expected });

      // A version without a What's new entry, or not a version, is refused
      const none = manifest(['1.2.4', archive, sig, notes]);
      assert.notEqual(none.status, 0);
      assert.match(none.stderr, /has no entry for 1\.2\.4/);
      assert.notEqual(manifest(['latest', archive, sig, notes]).status, 0);
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

  // release.sh tags origin/main's commit (ATM-705): what is checked out, and the working tree, change nothing
  describe('release.sh', () => {
    const repos = () => {
      const origin = tempDir('atm-origin-');
      const root = tempDir('atm-release-');
      git(['init', '-q', '--bare', '-b', 'main'], origin);
      git(['init', '-q', '-b', 'main'], root);
      git(['config', 'user.email', 't@example.com'], root);
      git(['config', 'user.name', 'T'], root);
      git(['remote', 'add', 'origin', origin], root);
      return { origin, root };
    };
    const commit = (root, version, message) => {
      fs.writeFileSync(path.join(root, 'Cargo.toml'), `[workspace.package]\nversion = "${version}"\n`);
      git(['add', 'Cargo.toml'], root);
      git(['commit', '-qm', message], root);
    };
    const cleanUp = ({ origin, root }) => {
      removeDir(origin);
      removeDir(root);
    };

    test('refuses what isn’t a version, and an unknown option', () => {
      const r = repos();
      try {
        commit(r.root, '1.2.3', 'one');
        git(['push', '-q', 'origin', 'main'], r.root);
        assert.match(run('release.sh', ['latest'], r.root).stderr, /latest isn't a version/);
        assert.match(run('release.sh', ['1.2.3', '--force'], r.root).stderr, /unknown option --force/);
        assert.equal(git(['tag', '--list'], r.root), '');
      } finally {
        cleanUp(r);
      }
    });

    test('tags origin/main’s commit from another branch, with changes in the working tree', () => {
      const r = repos();
      try {
        commit(r.root, '1.2.3', 'the release');
        git(['push', '-q', 'origin', 'main'], r.root);
        const main = git(['rev-parse', 'HEAD'], r.root);
        git(['switch', '-q', '-c', 'dev/1.3.0'], r.root);
        commit(r.root, '1.3.0', 'work on the next one');
        fs.writeFileSync(path.join(r.root, 'notes.txt'), 'untracked');
        fs.writeFileSync(path.join(r.root, 'Cargo.toml'), '[workspace.package]\nversion = "1.2.3"\n'); // uncommitted

        const out = run('release.sh', ['1.2.3'], r.root);
        assert.equal(out.status, 0, out.stderr);
        assert.match(out.stdout, /Tagged v1\.2\.3: origin\/main, [0-9a-f]+ the release\./);
        assert.match(out.stdout, /Push it to build the release: git push origin v1\.2\.3/);
        assert.equal(git(['rev-parse', 'v1.2.3^{commit}'], r.root), main);
        assert.equal(git(['branch', '--show-current'], r.root), 'dev/1.3.0');
        assert.equal(fs.readFileSync(path.join(r.root, 'notes.txt'), 'utf8'), 'untracked');
        assert.match(run('release.sh', ['1.2.3'], r.root).stderr, /v1\.2\.3 exists already/);
      } finally {
        cleanUp(r);
      }
    });

    test('tags origin/main even when the local main is behind it', () => {
      const r = repos();
      try {
        commit(r.root, '1.2.2', 'old');
        git(['push', '-q', 'origin', 'main'], r.root);
        git(['switch', '-q', '-c', 'dev/1.2.3'], r.root);
        commit(r.root, '1.2.3', 'the release');
        git(['push', '-q', 'origin', 'dev/1.2.3:main'], r.root); // merged on GitHub
        const merged = git(['rev-parse', 'HEAD'], r.root);
        git(['switch', '-q', 'main'], r.root);
        git(['update-ref', 'refs/remotes/origin/main', 'main'], r.root); // not fetched yet

        const out = run('release.sh', ['1.2.3'], r.root);
        assert.equal(out.status, 0, out.stderr);
        assert.equal(git(['rev-parse', 'v1.2.3^{commit}'], r.root), merged);
      } finally {
        cleanUp(r);
      }
    });

    test('refuses a version origin/main’s Cargo.toml doesn’t have, whatever the working tree says', () => {
      const r = repos();
      try {
        commit(r.root, '1.2.3', 'one');
        git(['push', '-q', 'origin', 'main'], r.root);
        fs.writeFileSync(path.join(r.root, 'Cargo.toml'), '[workspace.package]\nversion = "1.2.4"\n');
        git(['commit', '-qam', 'not pushed'], r.root);

        const out = run('release.sh', ['1.2.4'], r.root);
        assert.equal(out.status, 1);
        assert.match(out.stderr, /origin\/main's Cargo\.toml says 1\.2\.3, not 1\.2\.4: is the release PR merged\?/);
        assert.equal(git(['tag', '--list'], r.root), '');
      } finally {
        cleanUp(r);
      }
    });

    test('refuses a tag that exists only on origin', () => {
      const r = repos();
      try {
        commit(r.root, '1.2.3', 'one');
        git(['push', '-q', 'origin', 'main'], r.root);
        git(['tag', 'v1.2.3'], r.root);
        git(['push', '-q', 'origin', 'v1.2.3'], r.root);
        git(['tag', '-d', 'v1.2.3'], r.root);

        const out = run('release.sh', ['1.2.3'], r.root);
        assert.equal(out.status, 1);
        assert.match(out.stderr, /v1\.2\.3 exists already on origin/);
        assert.equal(git(['tag', '--list'], r.root), '');
      } finally {
        cleanUp(r);
      }
    });

    test('--push pushes the tag to origin', () => {
      const r = repos();
      try {
        commit(r.root, '1.2.3', 'one');
        git(['push', '-q', 'origin', 'main'], r.root);

        const out = run('release.sh', ['1.2.3', '--push'], r.root);
        assert.equal(out.status, 0, out.stderr);
        assert.match(out.stdout, /Pushed v1\.2\.3/);
        assert.equal(git(['tag', '--list'], r.origin), 'v1.2.3');
      } finally {
        cleanUp(r);
      }
    });
  });
});
