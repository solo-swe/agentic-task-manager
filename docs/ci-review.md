# CI review (ATM-487), November 2026

What CI taught us from the Rust port to 0.38.0, and the changes proposed for `.github/workflows/`. Each change is
Hakan's decision (editing CI is the user's call); the decisions to make are at the end, §4.

## 1. What we learned

- **Every "flaky" run was a real bug.** 0.38.0's three intermittent failures each had a cause in atm, not in the
  runner:
  - macOS, the stress test's lock timeouts: a waiter that kept losing the lock to others timed out while the lock
    kept changing hands (ATM-481, fixed: the wait starts again when the holder changes);
  - Linux, the app's page embedded half-written: two compiles of the app at once shared Tauri's asset cache
    (ATM-559, fixed in `build.rs`);
  - Windows, the suite (ATM-482): a claim's watcher followed pid 0 and outlived its shell; `atm service`'s port
    checks waited out each closed port (Windows retries a refused local connection for about 2 s), so the serve
    tests ran 400 s and were killed.

  `/atm-push` already re-runs a failure that looks flaky only once. The lesson to keep: **a re-run that passes still
  gets a task**, with the failing run's link.
- **Time goes to compiling Rust, three times, from nothing.** The last green run (43993509673): each Rust job ~8 min,
  of which `clippy` 1m40–2m40 and `cargo test` 2m10–3m15; on Linux also `cargo install cargo-deny` 1m34 (it compiles
  cargo-deny) and the webview's packages 33 s. No job caches anything. The e2e chain (`atm for the e2e suite` 52 s,
  then Playwright 4m14) is shorter than the Rust jobs, so a run takes as long as the slowest Rust job.
- **The e2e binary's job is right as it is.** Building `-p atm-cli` alone on 22.04 takes under a minute; reusing the
  Rust job's binary would make Playwright wait the full Rust job and run on a newer glibc.
- **The release workflows worked first time for every release** (0.32.0 → 0.38.0). Nothing to change; one risk below
  (§2.6).
- **Screenshots fail every run of a sprint that changes the UI**, until the release task regenerates them (as now:
  ATM-558 changed the Help and two hints, so every dispatched run this sprint is red on Playwright). That hides a real
  e2e failure among the expected ones.

## 2. Proposals

### 2.1 Cache the Rust build (saves ~3–4 min a run)
Cache `~/.cargo/registry`, `~/.cargo/git` and `target/` per OS, keyed on `Cargo.lock` and `rust-toolchain.toml`,
restored by prefix when they change. Two ways:
- **`Swatinem/rust-cache`** (third-party, the common choice): prunes the workspace's own crates and old builds, so
  the cache stays small; pinned by SHA like every action (I19).
- **`actions/cache`** (GitHub's own): no third party, but `target/` unpruned (the app makes it several GB on each
  OS; the repository's cache limit is 10 GB, so caches would evict each other).

Either way: **saved from `main` only** (`save-if: github.ref == 'refs/heads/main'`), so a PR or a dispatched branch
reads main's cache and never writes one another run could read (cache poisoning), and **never in `release.yml`**,
which keeps building from nothing. `docs/security-architecture.md` gets a line on it (I19).

### 2.2 cargo-deny in a job of its own
It runs after the Ubuntu job's suite today, serially, compiling cargo-deny each time. In its own job it runs in
parallel with the Rust jobs, off the critical path (~1.5 min less for the Ubuntu job). With 2.1 its binary is cached
too. Not a required check yet (the ruleset lists the Rust jobs): add it to the ruleset as `cargo-deny`.

### 2.3 Pin the runner images, and name the jobs by platform
`ubuntu-latest` moves to Ubuntu 26 (the annotation on every run); `macos-latest` and `windows-latest` move too,
unannounced to us. Pin `ubuntu-24.04`, `macos-15`, `windows-2025` (bumped by hand, like the container digests), and
name the matrix by platform, `Rust (Linux)`, `Rust (macOS)`, `Rust (Windows)`, so a new image doesn't rename a
required check. **This renames three required checks:** the ruleset on GitHub has to change in the same PR's merge
(`.github/rulesets/main.json` and the live ruleset, which Hakan applies).

### 2.4 A test that the ruleset names CI's jobs
`ci-workflow.test.ts`: every `required_status_checks` context in `.github/rulesets/main.json` is a job's name in
`ci.yml` (with the matrix expanded). Renaming a job then fails the unit tests instead of blocking merges.

### 2.5 Run on release branches, once
Today a push to `dev/*` runs nothing until dispatched (`gh workflow run CI --ref …`), and the PR runs on each push.
Proposal: run on `push` to `dev/**` too, and add
`concurrency: { group: ci-${{ github.ref }}, cancel-in-progress: true }` (not for `main`), so pushing twice cancels
the first run. With a PR open, a push then runs twice (the push and the PR's): leave `pull_request` for PRs from
other branches (Dependabot's) and drop it for `dev/**`, or keep dispatching by hand as now. **Hakan's call**; the
other proposals don't depend on it.

### 2.6 The Linux build's image (later, not now)
The release's Linux builds and the e2e binary run on `ubuntu-22.04` for glibc 2.35. GitHub retires runner images
around the end of their release's standard support (22.04's ends in April 2027; the date GitHub announces is the one
to watch). Before then: build them in a pinned container instead (as
`pnpm run build:atm-linux` already does for the screenshots, in the `rust` image, Debian 12, glibc 2.36: that drops
Ubuntu 22.04 users), or an older image (`rust:…-bullseye`, glibc 2.31). A task for the backlog, with the glibc floor
to choose.

### 2.7 Screenshots: one step on their own
Split `pnpm exec playwright test` into the functional specs and `visual`, as two steps, so a run whose only failures
are the screenshots says so in its step list (and the functional suite's result isn't hidden). The rule stays as it
is: baselines are regenerated in the release task. Alternative: regenerate in the task that changes the UI (the CI
artifact fallback makes that cheap); more commits of PNGs, no red runs mid-sprint. **Hakan's call.**

## 3. Not changing
- What the jobs check, and on which platforms (the suite on all three since ATM-482).
- `atm for the e2e suite` and the Playwright container (§1).
- The release workflows (§1), and their permissions: no permission or secret changes anywhere.

## 4. Decisions for Hakan
1. **2.1 cache:** `Swatinem/rust-cache` (third-party, pinned) or `actions/cache` (GitHub's own), or no cache?
2. **2.2** cargo-deny as its own job, and a required check?
3. **2.3** pin the images and rename the jobs `Rust (Linux)`, `Rust (macOS)`, `Rust (Windows)` (you update the live
   ruleset when it merges)?
4. **2.5** CI on pushes to `dev/**` with cancel-in-progress, or keep dispatching by hand?
5. **2.7** screenshots as their own step (regenerated in the release task, as now), or regenerated in the task that
   changes the UI?
6. **2.6** a backlog task for the Linux build's image before April 2027?

2.4 (the ruleset test) needs no decision: it comes with whichever names 2.3 settles on.
