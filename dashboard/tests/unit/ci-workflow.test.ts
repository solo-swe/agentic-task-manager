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

  it('every runner image is pinned, never a -latest label (ATM-487)', () => {
    const images = [
      ...lines.filter((line) => /^\s+runs-on: /.test(line) && !line.includes('${{')).map((l) => l.split(': ')[1]),
      ...[...workflow.matchAll(/\bos: ([\w.-]+)/g)].map((m) => m[1]),
    ];
    expect(images.length).toBeGreaterThan(0);
    for (const image of images) expect(image).toMatch(/^(ubuntu|macos|windows)-\d[\d.]*(-arm)?$/);
  });

  it('the build cache is saved from main only (ATM-487, I19)', () => {
    const caches = lines.flatMap((line, i) => (/uses: Swatinem\/rust-cache@/.test(line) ? [i] : []));
    expect(caches.length).toBeGreaterThan(0);
    for (const i of caches)
      expect(lines.slice(i + 1, i + 3).join('\n')).toMatch(/save-if: \$\{\{ github\.ref == 'refs\/heads\/main' \}\}/);
  });
});

// The ruleset requires checks by their names (I21): a job renamed in ci.yml would block every merge to main
describe('the ruleset names CI’s jobs (ATM-487)', () => {
  const ruleset = JSON.parse(readFileSync(new URL('../../../.github/rulesets/main.json', import.meta.url), 'utf8')) as {
    rules: { type: string; parameters?: { required_status_checks?: { context: string }[] } }[];
  };
  const required = ruleset.rules
    .flatMap((rule) => rule.parameters?.required_status_checks ?? [])
    .map((check) => check.context);
  /** Each job's name, a matrix's expanded by its `platform`s */
  const jobNames = workflow
    .split(/^ {2}(?=[\w-]+:\n)/m)
    .slice(1)
    .flatMap((job) => {
      const name = /^ {4}name: (.+)$/m.exec(job)?.[1];
      if (!name) return [];
      if (!name.includes('${{ matrix.platform }}')) return [name];
      return [...job.matchAll(/platform: ([\w-]+)/g)].map((m) => name.replace('${{ matrix.platform }}', m[1]));
    });

  it('every required check is one of ci.yml’s jobs', () => {
    expect(required.length).toBeGreaterThan(0);
    for (const check of required) expect(jobNames).toContain(check);
  });

  it('the jobs are named by platform, not by runner image', () => {
    expect(jobNames).toEqual(expect.arrayContaining(['Rust (Linux)', 'Rust (macOS)', 'Rust (Windows)', 'cargo-deny']));
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

  it('the release builds from nothing: no cache (ATM-487)', () => {
    for (const w of [release, homebrew]) expect(w).not.toMatch(/rust-cache|actions\/cache/);
  });

  it('releases come from v* tags only, and the formula from published releases', () => {
    expect(release).toMatch(/^on:\n {2}push:\n {4}tags: \["v\*"\]$/m);
    expect(homebrew).toMatch(/^on:\n {2}release:\n {4}types: \[published\]$/m);
  });

  // The tag is a plain `git tag` (ATM-705): the release checks it before building
  it('builds only once the tag is checked: the version Cargo.toml has, on a commit on main', () => {
    const job = (name: string) => release.split(/^ {2}(?=[\w-]+:\n)/m).find((j) => j.startsWith(`${name}:`)) ?? '';
    const tag = job('tag');
    expect(tag).toMatch(/fetch-depth: 0/);
    expect(tag).toMatch(/grep -qx "version = \\"\$version\\"" Cargo\.toml/);
    expect(tag).toMatch(/git merge-base --is-ancestor "\$GITHUB_SHA" origin\/main/);
    for (const name of ['build', 'app-macos']) expect(job(name)).toMatch(/^ {4}needs: tag$/m);
    expect(job('release')).toMatch(/^ {4}needs: \[build, app-macos\]$/m);
  });

  // ATM-661 (audit A1, A5, A11): the signing and deploy secrets are kept from everything that compiles
  const workflows = { 'release.yml': release, 'homebrew.yml': homebrew, 'skills.yml': read('skills.yml') };
  const jobsOf = (w: string) =>
    w
      .slice(w.indexOf('\njobs:\n'))
      .split(/^ {2}(?=[\w-]+:\n)/m)
      .slice(1);
  const stepsOf = (job: string) => job.split(/^ {6}- /m).slice(1);
  const compiles = (step: string) => /cargo build|--no-bundle/.test(step);

  for (const [name, w] of Object.entries(workflows)) {
    it(`${name}: a step with a secret comes after every step that compiles, and compiles nothing itself`, () => {
      for (const job of jobsOf(w)) {
        const steps = stepsOf(job);
        const secret = steps.findIndex((s) => /secrets\./.test(s));
        if (secret < 0) continue;
        const lastCompile = steps.reduce((last, s, i) => (compiles(s) ? i : last), -1);
        expect(lastCompile, job.split('\n')[0]).toBeLessThan(secret);
        for (const step of steps.filter((s) => /secrets\./.test(s))) expect(step).not.toMatch(/cargo build/);
      }
    });

    it(`${name}: every job with a secret is in the release environment (v* tags only)`, () => {
      const holding = jobsOf(w).filter((job) => /secrets\./.test(job));
      expect(holding.length).toBeGreaterThan(0);
      for (const job of holding) expect(job, job.split('\n')[0]).toMatch(/^ {4}environment: release$/m);
    });

    it(`${name}: every runner is pinned by version, never a -latest label`, () => {
      const runners = w.split('\n').filter((line) => /runs-on:|\bos: /.test(line));
      expect(runners.length).toBeGreaterThan(0);
      for (const line of runners) expect(line).not.toMatch(/-latest/);
    });
  }

  it('the app compiles with no secret before it is bundled and signed', () => {
    const app = jobsOf(release).find((j) => j.startsWith('app-macos:'))!;
    const steps = stepsOf(app);
    const compile = steps.findIndex((s) => /app:bundle --target universal-apple-darwin --no-bundle/.test(s));
    const sign = steps.findIndex((s) => /app:bundle --target universal-apple-darwin\n/.test(s));
    expect(compile).toBeGreaterThan(0);
    expect(steps[compile]).not.toMatch(/env:/);
    expect(sign).toBeGreaterThan(compile);
  });
});

describe('Dependabot (I19, T-275)', () => {
  const config = readFileSync(new URL('../../../.github/dependabot.yml', import.meta.url), 'utf8');
  // Each `- package-ecosystem:` entry, as text
  const ecosystems = config.split(/^ {2}- package-ecosystem: /m).slice(1);

  it('watches the actions, the dashboard npm packages and atm’s crates (ATM-678), weekly', () => {
    expect(ecosystems.map((e) => e.split('\n')[0])).toEqual(['github-actions', 'npm', 'cargo']);
    for (const e of ecosystems) expect(e).toMatch(/interval: weekly/);
  });

  it("groups each ecosystem's updates into one PR (npm and cargo: minor and patch; a major gets its own)", () => {
    const [actions, npm, cargo] = ecosystems;
    expect(actions).toMatch(/groups:\n {6}actions:\n {8}patterns: \['\*'\]/);
    expect(actions).not.toMatch(/update-types/);
    expect(npm).toMatch(/groups:\n {6}npm:\n {8}patterns: \['\*'\]\n {8}update-types: \[minor, patch\]/);
    expect(cargo).toMatch(/directory: \/\n/);
    expect(cargo).toMatch(/groups:\n {6}cargo:\n {8}patterns: \['\*'\]\n {8}update-types: \[minor, patch\]/);
  });
});

// ATM-678 (audit A12): the workflows that push with a deploy key run on a release's tag only, its own commit
// checked out, and trust github.com's published host keys and no other
describe('the deploy-key pushes (Homebrew, the skills export)', () => {
  const read = (name: string) => readFileSync(new URL(`../../../.github/workflows/${name}`, import.meta.url), 'utf8');
  const pushes = { 'homebrew.yml': read('homebrew.yml'), 'skills.yml': read('skills.yml') };

  for (const [name, w] of Object.entries(pushes)) {
    it(`${name}: refuses anything but a v<MAJOR>.<MINOR>.<PATCH> tag before anything else, and checks the tag out`, () => {
      const steps = w.slice(w.indexOf('    steps:'));
      expect(steps.indexOf("The tag is a release's")).toBeGreaterThan(0);
      expect(steps.indexOf("The tag is a release's")).toBeLessThan(steps.indexOf('uses: actions/checkout@'));
      expect(steps).toMatch(/case "\$TAG" in\n {12}v\[0-9\]\*\.\[0-9\]\*\.\[0-9\]\*\) ;;\n {12}\*\) .*exit 1 ;;/);
      expect(steps).toMatch(/ref: \$\{\{ github\.event\.release\.tag_name \|\| inputs\.tag \}\}/);
    });

    it(`${name}: ssh trusts the pinned host keys only, never a new one`, () => {
      expect(w).not.toMatch(/accept-new|StrictHostKeyChecking=no/);
      expect(w).toMatch(/-o UserKnownHostsFile=\$RUNNER_TEMP\/known_hosts -o StrictHostKeyChecking=yes"/);
      expect(w).toMatch(/printf '%s' "\$GITHUB_HOST_KEYS" > "\$RUNNER_TEMP\/known_hosts"/);
      const keys = w.split('\n').filter((line) => /^ {8}github\.com /.test(line));
      expect(keys.map((k) => k.trim().split(' ')[1])).toEqual(['ssh-ed25519', 'ecdsa-sha2-nistp256', 'ssh-rsa']);
      expect(keys[0]).toContain('AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl');
    });
  }

  it('both pin the same keys', () => {
    const keys = (w: string) => w.split('\n').filter((line) => /^ {8}github\.com /.test(line));
    expect(keys(pushes['skills.yml'])).toEqual(keys(pushes['homebrew.yml']));
  });
});

// ATM-678 (audit A11): the sandbox's images are pinned by digest, like CI's
describe('the agent sandbox’s images', () => {
  const read = (path: string) => readFileSync(new URL(`../../../tests/sandbox/${path}`, import.meta.url), 'utf8');

  it('the proxy and the agent’s base image are pinned by digest', () => {
    expect(read('compose.yaml')).toMatch(/^ {4}image: ubuntu\/squid:[\w.-]+@sha256:[0-9a-f]{64}$/m);
    expect(read('Dockerfile')).toMatch(/^FROM node:[\w.-]+@sha256:[0-9a-f]{64}$/m);
  });
});
