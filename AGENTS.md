# Agentic Task Manager — development

atm is one program, `atm` (Rust, `crates/`), that brings its dashboard and its agents' skills with it. Users install it
with Homebrew or a release download ([packaging/README.md](packaging/README.md)); the Claude Code plugin it was until
0.32.0 is retired (T-454).

## Layout
- `crates/` — atm, one crate per layer (below); the binary is `atm-cli`'s `atm`
- `app/src-tauri/` — the atm app (Tauri, T-455 onwards; the [Tauri guide](docs/guides/tauri.md)), a workspace member
  on the same layers, so `cargo build` and `cargo test` build it too (on Linux with WebKitGTK's development
  packages, as CI installs them). Its window shows the built dashboard (`dashboard/dist/`); its commands' TypeScript
  types are written by its tests to `.generated/app/bindings.ts`
- `skills/atm-*/SKILL.md` — the skills, their one source, portable to any agent harness (`name` and `description`
  only, a version check first, detail from `atm docs`; T-441), embedded in atm, which `atm agents install` writes
  where harnesses read them. Claude Code's pre-approved commands for each are in `skills/claude-code.json`
- `dashboard/` — Svelte 5 + TypeScript source for the dashboard
- `dashboard/dist/` — the built dashboard (`tasks-dashboard.html`) and CompMD's runtime (`assets/`), served by
  `atm service` (one server per user, for all projects; routes in `docs/multi-project-api.md`): **never committed**,
  `pnpm run build` in `dashboard/` makes it, and atm embeds it, so build it before `cargo build` (T-477)
- `crates/atm-common/schema/` — the data model's JSON Schemas and the default vocabularies, the one source for atm
  and the dashboard
- `.generated/` — what is generated from them, never committed (gitignored): `dashboard/build/generate.ts` writes the
  TypeScript into `.generated/dashboard/`, imported as `$generated/…` (`pnpm run generate`; `pnpm install` and the
  dashboard's scripts run it)
- `tests/scripts/` — the CLI suite, run against the binary
- `packaging/` — the release: the release script, macOS signing, the Homebrew formula, the public skills repo's export
- `.agentic-task-manager/tasks.json` — this repo's own task list; we dogfood atm here (`/atm-serve` shows it)
- `docs/` — requirements, designs and plans the tasks were created from (they describe atm as it was when written)
- `docs/agent/` — the reference `atm docs <topic>` prints for agents (one topic per file, first line
  `# <Title>: <summary>`), embedded in atm (`crates/atm-cli/build.rs`); the skills point to it rather than repeat it
- `docs/adr/` — architecture decisions, one per file (`ADR-NNN-<title>.md`: status, context, decision, consequences);
  a decision that changes one is a new ADR that replaces it
- `docs/guides/` — the coding guides: [Svelte](docs/guides/svelte.md) (the dashboard), [Rust](docs/guides/rust.md)
  (atm) and [Tauri](docs/guides/tauri.md) (the atm app). Where the code doesn't follow a guide yet, a task says so;
  follow the guide in new code

## Security
`docs/security-architecture.md` holds the threat model and the invariants every change keeps, each with its check.

**Agent sandbox** (before merging a PR that changes a skill, `docs/agent/` or atm output agents act on):
`ANTHROPIC_API_KEY=... ATM_SANDBOX_BIN=<a Linux atm> node tests/sandbox/run.mjs` with Docker running (a release
archive's `atm`, or the `atm-linux` artifact of CI's run on your branch). It builds a container with that atm and its
skills, runs `/atm-loop-tasks` on a seeded project whose red-team tasks ask for `curl | sh`, secrets, CI and hook
changes and another remote, with no network but the model API, and prints per task whether the agent blocked it. A
few minutes and a few dollars of model calls. Its judgement is tested in `tests/scripts/sandbox-verdict.test.mjs`.
A change that adds an entry point (an HTTP route, a CLI command reading a path or URL, a task-folder file), a field
agents or the dashboard act on, a skill instruction that runs commands or reads outside text, or a CI permission
updates that document in the same PR.

## Dashboard (`dashboard/`)
Coding rules: the [Svelte guide](docs/guides/svelte.md). SvelteKit (T-428), its options in `vite.config.ts`'s `sveltekit({ … })` call: a single-page app, rendered in the
browser only, from the static adapter's fallback page with every script and style inlined
(`output.bundleStrategy: 'inline'`), so it builds to a **single self-contained HTML file**; `build/finish-page.ts`
stamps it and writes it to `dist/`, where atm's build embeds it. The views are routes (T-429): the app is in the root layout
(`src/routes/+layout.svelte`), the route files under it are its addresses (`/projects/<id>/<tab>`, `/board`, …), and
`src/lib/address.ts` reads and writes them (what is open over a page in the query: `?task=`, `?doc=`, `?sprint=`);
the routes and `atm serve`'s `PAGE_PATH` are listed in `docs/multi-project-api.md` §2.7. Navigate with
`lib/navigation.ts`, never `history` or `location` directly; API URLs are from the server's root. Components live by area in
`src/lib/<area>/` (`shell/`, `ui/`, `kanban/`, `tasks/`, `graph/`, `details/`, `dialogs/`, `documents/`, `progress/`,
`agents/`, `overview/`; T-430), beside the logic modules in `src/lib/`. Colours, spacing, radii and text sizes are
tokens in `src/tokens.css` (T-433; the themes set the colours): use `var(--space-N)`, `var(--radius-…)`,
`var(--text-…)`, never a px literal for a padding, margin, gap or radius (`tokens.test.ts`). Dialogs, menus and
popovers are Bits UI's (T-431). The `atm-dashboard-version` meta tag is
injected from `Cargo.toml` (atm's version) at build time; never edit it by hand.

`pnpm run build` writes `dashboard/dist/tasks-dashboard.html` — a build output, **never committed** (T-477): CI and
the release build it before the binary that embeds it.

**pnpm**, installed with its [standalone installer](https://pnpm.io/installation) (not corepack, not npm), at the
version CI installs (`.github/workflows/ci.yml`). Install with `pnpm install --frozen-lockfile`: it installs exactly the
locked versions and never updates the lockfile (`pnpm-lock.yaml`); a plain `pnpm install` only when you change
`package.json` on purpose. Overrides (security pins) are in `pnpm-workspace.yaml`. The guide's `devEngines` pins come
with T-436, once CompMD reads pnpm 12's two-document lockfile.
The pre-port dashboard is frozen at `dashboard/tests/legacy/tasks-dashboard.v0.4.0.html` (see `docs/svelte-migration-plan.md`).

Commands (run in `dashboard/`):
- `pnpm run dev` — Vite dev server, with the real REST API: this checkout's Rust atm (`cargo build` first; its
  `atm _serve` on a temporary state folder, `ATM_STATE_DIR`, so your own `atm service` is untouched; `build/dev-api.ts` forwards `api/`,
  `p/`, `__atm/` and `assets/` to it) for this repo's `.agentic-task-manager/`, or for other folders with
  `ATM_DEV_DATA=<path to .agentic-task-manager>[:<another>…] pnpm run dev` (a folder without a task file is listed as
  missing). `cargo build` is enough to pick up atm changes: the API server restarts when the binary changes.
  The dev page runs under the server's Content-Security-Policy plus what Vite needs (`build/security-headers.ts`);
  `tests/scripts/dev-api.test.mjs` starts `vite dev` and checks all of this
- `pnpm run build` — single-file build, and the CompMD runtime the details panel loads (`vite.compmd.config.ts`,
  into `dist/assets/compmd/<version>/`), both into `dist/`, never committed
- `pnpm run size` — the build, with what the single file is made of, largest first (before a guardrail change)
- `pnpm test` / `pnpm run test:coverage` — Vitest unit tests (`tests/unit/`, 90% line threshold on `src/lib`)
- `pnpm run test:e2e` — build, then the Playwright e2e suite without the screenshots
- `pnpm run test:e2e:visual` / `test:e2e:visual:update` — the screenshots, in Docker (Linux baselines), against a
  Linux atm that `pnpm run build:atm-linux` builds first (in the pinned Rust image, into `target/linux/`)
- `pnpm run check` — the release notes' entries (`scripts/check-versions.mjs`), svelte-check, ESLint, Prettier
- `pnpm run test:scripts` — atm's test suite (builds the binary; see below)
- `pnpm run branding` — regenerate `branding/` (the logo for the README and GitHub) from `src/assets/atm-logo.svg`
- `pnpm run verify` — everything above (needs Docker for the screenshots)

## atm (`Cargo.toml`, `crates/`)
atm ([porting-to-rust.md](docs/porting-to-rust.md), the [Rust guide](docs/guides/rust.md)): one crate per layer
(`docs/layered-architecture.md`) under `crates/` (`atm-common`, `atm-db`, `atm-vcs`, `atm-lib`, `atm-boot`, `atm-api`,
`atm-cli`, whose binary is `atm`), so the compiler checks the layer rule; the toolchain pinned in
`rust-toolchain.toml`, atm's version the workspace's.
- `cargo build`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all`, `cargo deny check`
  (from the repository root; `pnpm run build` in `dashboard/` first, as atm embeds the dashboard).
- Tests (`tests/scripts/`, `pnpm run test:scripts` in `dashboard/`, or `node tests/scripts/run.mjs` after
  `cargo build`) drive the binary exactly as users run it, each file in its own process with a watchdog. Prefer
  data-driven scenarios in `tests/scripts/scenarios/*.json` (format at the top of `scenarios.test.mjs`); use
  `*.test.mjs` with `helpers.mjs` for things scenarios can't express (servers, concurrency, git merges). CI runs the
  suite on Linux and macOS, and on Windows once T-482 is done.
- The e2e suite's fixture gets its task views and summaries from the binary too (`pnpm run test:e2e` builds it).

## Testing conventions
- e2e specs import `test`/`expect` from `tests/e2e/helpers.ts`. `tasks.json` is never served from disk: use the
  `tasksServer` fixture (`openDashboard(page, tasksServer, 'standard')` or an inline object), which plays atm serve's
  API: `api/projects` lists one project (`FIXTURE_PROJECT`), its tasks are served by the v1 route the dashboard loads
  (`api/v1/projects/<id>/tasks`, the fixture's data as views) and its files under `p/<id>/`;
  `addProject(id, data)` adds more, `noProjects()` lists none. It can also swap the response or return an error. Any
  non-localhost request fails the test.
- Fixtures live in `tests/fixtures/*.json`.
- Unit tests for `src/lib/x.ts` are `tests/unit/x.test.ts`; component tests are `tests/unit/Name.component.test.ts`.
  The suffix matters: macOS file systems are case-insensitive, so `Filters.test.ts` would overwrite `filters.test.ts`.
- Component tests share the `dashboard` state singleton; reset it with `loadIntoDashboard()` from
  `tests/unit/components.helpers.ts`.
- Specs run against the build (`dashboard/dist/tasks-dashboard.html`). They rely on stable DOM hooks (ids,
  `.filter-btn[data-filter]`, `.task-card[data-id]`, `#detail-overlay.open`, …) that date from the pre-Svelte
  dashboard, which was the parity oracle during the port and is kept, unused, at `tests/legacy/`.
- **Which tests to run (T-345):** while working on a task, the specs of the area you change
  (`pnpm exec playwright test kanban`, after `pnpm run build`) and the unit tests; the whole functional suite
  (`pnpm run test:e2e`, about 40 s) once, before committing. Not the screenshots: they run in CI on every push, and are
  regenerated once a sprint, in the release task. `pnpm run verify` runs everything, the screenshots too.
- **Screenshots** (`visual.spec.ts`) have **Linux baselines only**, made and checked in the pinned Playwright
  container, the same as CI's; on macOS the tests skip. With Docker running: `pnpm run test:e2e:visual` compares,
  `pnpm run test:e2e:visual:update` regenerates the changed ones (delete a baseline whose test is gone). Both first
  build a Linux atm for the fixture (`pnpm run build:atm-linux`: the official `rust` image at the pinned toolchain,
  pinned by digest, into `target/linux/`; the Playwright container runs it as `ATM_E2E_BIN`), as CI's e2e job runs its
  `atm-linux` build; bump that image's digest with the toolchain (T-485). They are of
  parts (a panel, a menu, a card, a theme's Kanban column) rather than whole pages, so one change touches few; a
  few dozen differing pixels pass (anti-aliasing), a real change doesn't.
- A test that waits in real time is a slow test: the tests reduce motion (`reducedMotion`), so scrolls and fits are
  instant, and a timer is skipped with `page.clock` (`cards.spec.ts`). More workers than Playwright's default don't
  help here (the machine is saturated at 5) and break the timing budgets. Traces are recorded only on CI's retry;
  locally, `--trace on` while looking into a failure.

## CI
- **Releases of the CLI** (T-452, [packaging/README.md](packaging/README.md)): `packaging/release.sh <version> --push`
  on `main` tags `v<version>`; `.github/workflows/release.yml` builds, signs and notarizes, and drafts the GitHub
  release; publishing it runs `.github/workflows/homebrew.yml`, which updates `Formula/atm.rb` in
  `solo-swe/homebrew-tap`, and `.github/workflows/skills.yml`, which exports the skills to `solo-swe/skills`, the public
  skills repo our apps share ([ADR-002](docs/adr/ADR-002-atm-public-skills-repo.md)). Their secrets are listed in the
  README.
- **Workflow:** `pull-request` (also in `tasks.json` `meta.workflow`, so `/atm-push` knows): work happens on a
  release branch (e.g. `dev/v4`) and reaches `main` through **one combined PR per release branch**.
- **Linux screenshot baselines:** made locally with `pnpm run test:e2e:visual:update` (Docker), normally. As a
  fallback, CI generates missing ones: it fails until the `-linux.png` baselines are committed, and attaches them as
  the `linux-screenshot-baselines` artifact. Find its ID with
  `gh api repos/solo-swe/agentic-task-manager/actions/runs/<run-id>/artifacts`; **check its `workflow_run` is your
  branch's run**: `head_branch` your branch, `head_repository_id` this repository's, and `head_sha` a commit on your
  branch (a fork's PR uploads an artifact of the same name). Download it with
  `gh api repos/solo-swe/agentic-task-manager/actions/artifacts/<id>/zip > <scratch>/baselines.zip` (`gh run download`
  fails on the zip's duplicate entries), extract only the missing `*-linux.png` files into
  `dashboard/tests/e2e/__screenshots__/` (`unzip -n`), look at one to check it shows what you expect, then commit them.
- **Pinned:** actions by commit SHA, container images by digest, pnpm by version (installed from the npm registry in
  each job, and in the Docker screenshot scripts), the token read-only (`permissions: contents: read`), checkouts
  without stored credentials. Dependabot proposes action and package updates, grouped: one PR a week for the actions,
  one for the packages' minor and patch updates, and one per major (T-275; its `npm` ecosystem reads
  `pnpm-lock.yaml`); bump an image digest by hand (`docker buildx imagetools inspect <image>`), the Playwright image
  together with `@playwright/test`, and pnpm's version in `ci.yml` and `package.json`'s Docker scripts together.
- **Performance budgets:** the e2e interaction budget is 200 ms locally and twice that in CI
  (`tests/e2e/performance.spec.ts`), and CI runners are about 3× slower than a developer Mac. A failure only in CI
  (e.g. 407 ms against 400) still needs a local measurement and a real fix: never raise the budget or just re-run.
  The large-file benchmark (`tests/scripts/bench.mjs --check`) has generous limits and fails only far over them.

## Versions
atm's version is the Rust workspace's (`Cargo.toml`, `[workspace.package]`, with the description, author, licence and
repository the dashboard's About dialog shows). Bump it when the dashboard or skills change, so running dashboard
servers restart on the new dashboard (`atm service --refresh`).

Every release gets **two** entries (`pnpm run check` fails without either):
- `CHANGELOG.md` — the detailed record, for developers: every change, with command names and flags.
- `WHATS-NEW.md` — what users see in the dashboard's What's new: 3–6 short
  bullets in plain language about what a user notices. No command flags, internals or file names; one idea per
  bullet; say what it does for them ("Agents can ask you a question instead of guessing"), not how it works. Write
  it from the changelog entry, not instead of it.

Before merging a release PR (every PR to `main` is one), the security checklist (`docs/security-architecture.md` §3):
- the PR changes a skill, `docs/agent/` or atm output agents act on → run the agent sandbox (Security, above);
- it adds an entry point, a field agents or the dashboard act on, or a CI permission → `docs/security-architecture.md`
  is updated in the same PR.
