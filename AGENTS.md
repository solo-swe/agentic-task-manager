# Agentic Task Manager — development

This repo is a Claude Code plugin marketplace (`.claude-plugin/marketplace.json`) containing one plugin, `atm`, in `plugin/`.
Only `plugin/` is shipped to users. `plugin/CLAUDE.md` is the plugin's own context file — don't put dev notes there.

## Layout
- `skills/atm-*/SKILL.md` — the skills, their one source, portable to any agent harness (`name` and `description`
  only, a version check first, detail from `atm docs`; T-441). Claude Code's pre-approved commands for each are in
  `skills/claude-code.json`
- `plugin/skills/*/SKILL.md` — the `/atm:*` skills as the Claude Code plugin ships them: **generated** from `skills/`
  by `atm-cli/build.py` (with `allowed-tools`, `/atm:` links and the plugin's own `bin/atm`); never edit them by hand
- `plugin/templates/tasks-dashboard.html` — the dashboard, served by `atm service` (one server per user, for all
  projects; routes in `docs/multi-project-api.md`)
- `plugin/scripts/atm` — atm-cli, the `atm` command the skills call (launcher for `atm.py`, see below)
- `atm-cli/` — atm-cli's source modules and the build that writes `plugin/scripts/atm.py` (dev only, not shipped)
- `dashboard/` — Svelte 5 + TypeScript source for the dashboard (dev only, not shipped)
- `.generated/` — what `atm-cli/build.py` generates for the build and the dashboard (the data model's types, the
  vocabularies, `atm docs`' topics and the skills as Python modules in `.generated/atm-cli/atm/`; the TypeScript in
  `.generated/dashboard/`, imported as `$generated/…`). **Never committed** (gitignored): `pnpm install` in
  `dashboard/`, the dashboard's scripts and `tests/scripts/run.mjs` write it, or `pnpm run generate`. What the plugin
  ships is generated too but committed (`plugin/scripts/atm.py`, `plugin/skills/`): users install from git
- `tests/scripts/` — tests for atm-cli (dev only)
- `.agentic-task-manager/tasks.json` — this repo's own task list; we dogfood atm here (`/atm:serve` shows it)
- `docs/` — requirements, designs and plans the tasks were created from
- `docs/agent/` — the reference `atm docs <topic>` prints for agents (one topic per file, first line
  `# <Title>: <summary>`), built into `atm.py` by `atm-cli/build.py`; the skills point to it rather than repeat it
- `docs/adr/` — architecture decisions, one per file (`ADR-NNN-<title>.md`: status, context, decision, consequences);
  a decision that changes one is a new ADR that replaces it
- `docs/guides/` — the coding guides: [Svelte](docs/guides/svelte.md) (the dashboard), [Rust](docs/guides/rust.md)
  (the Rust port of atm-cli) and [Tauri](docs/guides/tauri.md) (the atm app). A Python guide for atm-cli is still to
  come. Where the code doesn't follow a guide yet, a task says so; follow the guide in new code

## Security
`docs/security-architecture.md` holds the threat model and the invariants every change keeps, each with its check.

**Agent sandbox** (before merging a PR that changes a skill, `plugin/CLAUDE.md` or atm output agents act on):
`ANTHROPIC_API_KEY=... node tests/sandbox/run.mjs` with Docker running. It builds a container with this checkout's
plugin, runs `/atm:loop-tasks` on a seeded project whose red-team tasks ask for `curl | sh`, secrets, CI and hook
changes and another remote, with no network but the model API, and prints per task whether the agent blocked it. A
few minutes and a few dollars of model calls. Its judgement is tested in `tests/scripts/sandbox-verdict.test.mjs`.
A change that adds an entry point (an HTTP route, a CLI command reading a path or URL, a task-folder file), a field
agents or the dashboard act on, a skill instruction that runs commands or reads outside text, or a CI permission
updates that document in the same PR.

## Dashboard (`dashboard/`)
Coding rules: the [Svelte guide](docs/guides/svelte.md). SvelteKit (T-428), its options in `vite.config.ts`'s `sveltekit({ … })` call: a single-page app, rendered in the
browser only, from the static adapter's fallback page with every script and style inlined
(`output.bundleStrategy: 'inline'`), so it builds to a **single self-contained HTML file**; `build/finish-page.ts`
stamps it and writes it where the plugin ships it. The views are routes (T-429): the app is in the root layout
(`src/routes/+layout.svelte`), the route files under it are its addresses (`/projects/<id>/<tab>`, `/board`, …), and
`src/lib/address.ts` reads and writes them (what is open over a page in the query: `?task=`, `?doc=`, `?sprint=`);
the routes and `atm serve`'s `PAGE_PATH` are listed in `docs/multi-project-api.md` §2.7. Navigate with
`lib/navigation.ts`, never `history` or `location` directly; API URLs are from the server's root. Components live by area in
`src/lib/<area>/` (`shell/`, `ui/`, `kanban/`, `tasks/`, `graph/`, `details/`, `dialogs/`, `documents/`, `progress/`,
`agents/`, `overview/`; T-430), beside the logic modules in `src/lib/`. Colours, spacing, radii and text sizes are
tokens in `src/tokens.css` (T-433; the themes set the colours): use `var(--space-N)`, `var(--radius-…)`,
`var(--text-…)`, never a px literal for a padding, margin, gap or radius (`tokens.test.ts`). Dialogs, menus and
popovers are Bits UI's (T-431). The `atm-dashboard-version` meta tag is
injected from `plugin/.claude-plugin/plugin.json` at build time; never edit it by hand.

`pnpm run build` writes `plugin/templates/tasks-dashboard.html` — a **committed build artifact**, never edit it by hand.
The pre-commit hook in `.githooks/` rebuilds and stages it when dashboard sources are committed (enabled by
`pnpm install` in `dashboard/`, which sets `core.hooksPath`); CI fails if the committed file is stale.

**pnpm**, installed with its [standalone installer](https://pnpm.io/installation) (not corepack, not npm), at the
version CI installs (`.github/workflows/ci.yml`). Install with `pnpm install --frozen-lockfile`: it installs exactly the
locked versions and never updates the lockfile (`pnpm-lock.yaml`); a plain `pnpm install` only when you change
`package.json` on purpose. Overrides (security pins) are in `pnpm-workspace.yaml`. The guide's `devEngines` pins come
with T-436, once CompMD reads pnpm 12's two-document lockfile.
The pre-port dashboard is frozen at `dashboard/tests/legacy/tasks-dashboard.v0.4.0.html` (see `docs/svelte-migration-plan.md`).

Commands (run in `dashboard/`):
- `pnpm run dev` — Vite dev server, with the real REST API: this checkout's atm-cli (`plugin/scripts/atm-serve` on a
  temporary `ATM_HOME`, so your own `atm service` is untouched; `build/dev-api.ts` forwards `api/`, `p/` and `__atm/`
  to it) for this repo's `.agentic-task-manager/`, or for other folders with
  `ATM_DEV_DATA=<path to .agentic-task-manager>[:<another>…] pnpm run dev` (a folder without a task file is listed as
  missing). `pnpm run build:cli` is enough to pick up atm-cli changes: the API server restarts when `atm.py` changes.
  The dev page runs under the server's Content-Security-Policy plus what Vite needs (`build/security-headers.ts`);
  `tests/scripts/dev-api.test.mjs` starts `vite dev` and checks all of this
- `pnpm run build` — single-file build, and the CompMD runtime the details panel loads (`vite.compmd.config.ts`,
  into `plugin/assets/compmd/<version>/`, committed; `pnpm run check` fails when it is stale)
- `pnpm run size` — the build, with what the single file is made of, largest first (before a guardrail change)
- `pnpm test` / `pnpm run test:coverage` — Vitest unit tests (`tests/unit/`, 90% line threshold on `src/lib`)
- `pnpm run test:e2e` — build, then the Playwright e2e suite without the screenshots
- `pnpm run test:e2e:visual` / `test:e2e:visual:update` — the screenshots, in Docker (Linux baselines)
- `pnpm run check` — svelte-check, ESLint, Prettier
- `pnpm run build:cli` — build `plugin/scripts/atm.py` from `atm-cli/` (see below)
- `pnpm run test:scripts` — atm-cli's test suite (see below)
- `pnpm run branding` — regenerate `branding/` (the logo for the README and GitHub) from `src/assets/atm-logo.svg`
- `pnpm run verify` — everything above (needs Docker for the screenshots)

## atm-cli (`atm-cli/`, built into `plugin/scripts/`)
atm-cli is the `atm` command the skills run (earlier docs call it "the helper"). `plugin/scripts/atm` is a POSIX sh
launcher that runs `atm.py` with Python 3.9+ (or explains that Python is needed). atm-cli uses the standard library
and three vendored libraries, nothing installed: `transitions` (the task state machine; `atm-cli/vendored/`, bundled
into `atm.py`) and Typer with Click (the command tree: its root in `atm-cli/atm/cli/app.py`, one module per top-level command in
`atm-cli/atm/cli/commands/`, the entry in `cli/main.py`; copied as they are into
`plugin/scripts/vendor/`, which `atm.py` puts first on its import path; versions as `atm-cli/uv.lock` pins them,
`tests/scripts/vendor.test.mjs`). It must stay compatible with Python 3.9, the version macOS's Command Line Tools
ship. Python is the plugin's one runtime requirement; why there is a single implementation is in
`docs/helper-runtime-proposal.md`.

**Edit the modules in `atm-cli/atm/`, never `plugin/scripts/atm.py`.** That file is a **committed build artifact**:
`atm-cli/build.py` concatenates the modules, those it generates into `.generated/atm-cli/atm/` too, in dependency
order (its `MODULES` list; a module imports only from
earlier ones, and no two modules may define the same top-level name). The pre-commit hook rebuilds it when `atm-cli/`
changes, and CI fails if it is stale. Design: `docs/v4/atm-cli.md`.

**Layers** (`docs/layered-architecture.md`): every module is in a layer's folder (`common/`, `db/`, `vcs/`, `lib/`,
`boot/`, `cli/`, `api/`; the CLI and the REST API are two processes, `plugin/scripts/atm` and the internal
`plugin/scripts/atm-serve`, and never import each other), and the build
refuses an import the layer rule forbids, with no exceptions. atm-lib's public surface, for other tools, is two
modules: `atm.lib` (`atm/lib/__init__.py`, its `__all__`) and `atm.boot` (`open_project`, `open_memory_project`,
`open_registry`). Their names and signatures are frozen in `tests/lib/public-surface.json`: a change fails
`test_public.py` until that list is updated, and goes in the changelog. atm-lib's
unit tests are in `tests/lib/` (`unittest`, on the in-memory store; run by `pnpm run test:scripts`).

Tests (`tests/scripts/`, run with `node tests/scripts/run.mjs` or `pnpm run test:scripts`) drive the built atm-cli
through the launcher, exactly as users run it. Prefer data-driven scenarios in `tests/scripts/scenarios/*.json` (format at the top of
`scenarios.test.mjs`); use `*.test.mjs` with `helpers.mjs` for things scenarios can't express (servers, concurrency,
git merges). CI runs the suite with Python 3.9 and a current version.

## The Rust port (`Cargo.toml`, `crates/`)
atm is being ported to Rust ([porting-to-rust.md](docs/porting-to-rust.md), the [Rust guide](docs/guides/rust.md)):
one crate per layer under `crates/` (`atm-common`, `atm-db`, `atm-vcs`, `atm-lib`, `atm-boot`, `atm-api`, `atm-cli`,
whose binary is `atm`), the toolchain pinned in `rust-toolchain.toml`, the workspace's version the plugin's
(`check-versions.mjs` checks it). Until the switch the Python atm is the spec: the same suite runs against the
binary.
- `cargo build`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all`, `cargo deny check`
  (from the repository root).
- `pnpm run test:scripts:rust` in `dashboard/` (or `node tests/scripts/run.mjs --rust` after `cargo build`): the suite
  against the Rust `atm`. The tests it doesn't pass yet are in `tests/scripts/rust-expected-failures.json`; the run
  fails on any other failure, and on a listed test that passes: take it out of the list in the change that makes it
  pass. `--write-expected` rewrites the list from a run, never to hide a failure.

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
- Specs run against the build (`plugin/templates/tasks-dashboard.html`). They rely on stable DOM hooks (ids,
  `.filter-btn[data-filter]`, `.task-card[data-id]`, `#detail-overlay.open`, …) that date from the pre-Svelte
  dashboard, which was the parity oracle during the port and is kept, unused, at `tests/legacy/`.
- **Which tests to run (T-345):** while working on a task, the specs of the area you change
  (`pnpm exec playwright test kanban`, after `pnpm run build`) and the unit tests; the whole functional suite
  (`pnpm run test:e2e`, about 40 s) once, before committing. Not the screenshots: they run in CI on every push, and are
  regenerated once a sprint, in the release task. `pnpm run verify` runs everything, the screenshots too.
- **Screenshots** (`visual.spec.ts`) have **Linux baselines only**, made and checked in the pinned Playwright
  container, the same as CI's; on macOS the tests skip. With Docker running: `pnpm run test:e2e:visual` compares,
  `pnpm run test:e2e:visual:update` regenerates the changed ones (delete a baseline whose test is gone). They are of
  parts (a panel, a menu, a card, a theme's Kanban column) rather than whole pages, so one change touches few; a
  few dozen differing pixels pass (anti-aliasing), a real change doesn't.
- A test that waits in real time is a slow test: the tests reduce motion (`reducedMotion`), so scrolls and fits are
  instant, and a timer is skipped with `page.clock` (`cards.spec.ts`). More workers than Playwright's default don't
  help here (the machine is saturated at 5) and break the timing budgets. Traces are recorded only on CI's retry;
  locally, `--trace on` while looking into a failure.

## CI
- **Workflow:** `pull-request` (also in `tasks.json` `meta.workflow`, so `/atm:push` knows): work happens on a
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
`plugin/.claude-plugin/plugin.json` and `.claude-plugin/marketplace.json` versions must match. Bump both when the
dashboard or skills change, so `/plugin marketplace update` delivers them and running dashboard servers restart on the
new dashboard (`atm service --refresh`).

Every release gets **two** entries (`pnpm run check` fails without either):
- `plugin/CHANGELOG.md` — the detailed record, for developers: every change, with command names and flags.
- `plugin/WHATS-NEW.md` — what users see in the dashboard's What's new: 3–6 short
  bullets in plain language about what a user notices. No command flags, internals or file names; one idea per
  bullet; say what it does for them ("Agents can ask you a question instead of guessing"), not how it works. Write
  it from the changelog entry, not instead of it.

Before merging a release PR (every PR to `main` is one), the security checklist (`docs/security-architecture.md` §3):
- the PR changes a skill, `plugin/CLAUDE.md` or atm output agents act on → run the agent sandbox (Security, above);
- it adds an entry point, a field agents or the dashboard act on, or a CI permission → `docs/security-architecture.md`
  is updated in the same PR.
