// @vitest-environment node
// The CI workflow keeps its hardening (docs/security-architecture.md I18, I19; T-230): a read-only token, no stored
// credentials, actions pinned by commit SHA and container images by digest; the release workflows the same, but for
// the one job that drafts a release; and Dependabot's updates come grouped.
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const workflow = readFileSync(new URL('../../../.github/workflows/ci.yml', import.meta.url), 'utf8');
const lines = workflow.split('\n');

describe('CI workflow hardening (T-230)', () => {
  it('the token can only read the repository', () => {
    expect(workflow).toMatch(/^permissions:\n {2}contents: read$/m);
  });

  it('every action is pinned to a full commit SHA, with its version as a comment', () => {
    const uses = lines.filter((line) => /^\s*-?\s*uses:/.test(line));
    expect(uses.length).toBeGreaterThan(0);
    for (const line of uses) expect(line).toMatch(/uses: [\w.-]+\/[\w.-]+@[0-9a-f]{40} # v\d+/);
  });

  it('every checkout drops its credentials', () => {
    const checkouts = lines.flatMap((line, i) => (/uses: actions\/checkout@/.test(line) ? [i] : []));
    expect(checkouts.length).toBeGreaterThan(0);
    for (const i of checkouts) expect(lines.slice(i + 1, i + 3).join('\n')).toMatch(/persist-credentials: false/);
  });

  it('pnpm comes at a fixed version, and installs exactly the lockfile (T-427)', () => {
    const installs = lines.filter((line) => /pnpm install|npm (ci|install)/.test(line));
    expect(installs.length).toBeGreaterThan(0);
    for (const line of installs) {
      if (/npm install --global/.test(line)) expect(line).toMatch(/pnpm@\d+\.\d+\.\d+$/);
      else expect(line).toMatch(/pnpm install --frozen-lockfile$/);
    }
  });

  it('installs no Python packages: Typer and Click are vendored (T-425)', () => {
    expect(lines.filter((line) => /pip install/.test(line))).toEqual([]);
  });

  it('every container image is pinned by digest', () => {
    const images = lines.filter((line) => /^\s*(container|image):/.test(line) && !line.includes('${{'));
    expect(images.length).toBeGreaterThan(0);
    for (const line of images) expect(line).toMatch(/@sha256:[0-9a-f]{64}\s*$/);
  });
});

// The release workflows (T-452): read-only like CI, but for the one job that drafts the release, which builds nothing
describe('release workflows (I18, I19; T-452)', () => {
  const read = (name: string) => readFileSync(new URL(`../../../.github/workflows/${name}`, import.meta.url), 'utf8');
  const release = read('release.yml');
  const homebrew = read('homebrew.yml');

  it('the token is read-only, and only the job that drafts the release can write (its contents)', () => {
    for (const w of [release, homebrew]) expect(w).toMatch(/^permissions:\n {2}contents: read$/m);
    const grants = release.split('\n').filter((line) => /^\s+contents: write$/.test(line));
    expect(grants).toHaveLength(1);
    const job = release.slice(release.indexOf('\n  release:'));
    expect(job).toMatch(/^ {4}permissions:\n {6}contents: write$/m);
    expect(job).not.toMatch(/cargo |run: sh |npm |pnpm /);
    expect(homebrew).not.toMatch(/contents: write/);
  });

  it('every action is pinned to a full commit SHA, and every checkout drops its credentials', () => {
    for (const w of [release, homebrew]) {
      const lines = w.split('\n');
      const uses = lines.filter((line) => /^\s*-?\s*uses:/.test(line));
      expect(uses.length).toBeGreaterThan(0);
      for (const line of uses) expect(line).toMatch(/uses: [\w.-]+\/[\w.-]+@[0-9a-f]{40} # v\d+/);
      lines.forEach((line, i) => {
        if (/uses: actions\/checkout@/.test(line))
          expect(lines.slice(i + 1, i + 3).join('\n')).toMatch(/persist-credentials: false/);
      });
    }
  });

  it('releases come from v* tags only, and the formula from published releases', () => {
    expect(release).toMatch(/^on:\n {2}push:\n {4}tags: \["v\*"\]$/m);
    expect(homebrew).toMatch(/^on:\n {2}release:\n {4}types: \[published\]$/m);
  });
});

describe('Dependabot (I19, T-275)', () => {
  const config = readFileSync(new URL('../../../.github/dependabot.yml', import.meta.url), 'utf8');
  // Each `- package-ecosystem:` entry, as text
  const ecosystems = config.split(/^ {2}- package-ecosystem: /m).slice(1);

  it('watches the actions and the dashboard npm packages, weekly', () => {
    expect(ecosystems.map((e) => e.split('\n')[0])).toEqual(['github-actions', 'npm']);
    for (const e of ecosystems) expect(e).toMatch(/interval: weekly/);
  });

  it("groups each ecosystem's updates into one PR (npm: minor and patch; a major gets its own)", () => {
    const [actions, npm] = ecosystems;
    expect(actions).toMatch(/groups:\n {6}actions:\n {8}patterns: \['\*'\]/);
    expect(actions).not.toMatch(/update-types/);
    expect(npm).toMatch(/groups:\n {6}npm:\n {8}patterns: \['\*'\]\n {8}update-types: \[minor, patch\]/);
  });
});
