# Rust guide

atm's own Rust rules, for the Rust port of atm-cli ([porting-to-rust.md](../porting-to-rust.md)) and the app
([better-way-to-distribute-atm.md](../better-way-to-distribute-atm.md)). General Rust idioms apply on top of these;
[AGENTS.md](../../AGENTS.md) has the development notes for the whole repository.

## Toolchain and lints

- **Pinned toolchain** in `rust-toolchain.toml`. Upgrades happen in their own commit.
- **Edition 2024**, set once in `[workspace.package]`.
- **Lints** are set in the workspace `Cargo.toml` and inherited with `[lints] workspace = true`:
  - `unsafe_code` is forbidden;
  - `clippy::all` and `clippy::pedantic` are on, with the few noisy pedantic lints allowed there, never in code;
  - warnings fail CI.
- `#[allow(...)]` in code needs a short reason next to it.

## Crates and modules

- **Layer rules** come from [layered-architecture.md](../layered-architecture.md): the layers are `common`, `db`,
  `vcs`, `lib`, `boot`, `cli` and `api`, each importing only what that document allows; the CLI and the REST API are
  two front ends that never depend on each other. cargo-deny checks this, as `atm-cli/build.py` does for the Python
  modules today.
- **Module files:** `foo.rs` next to a `foo/` directory, never `foo/mod.rs`.
- **Visibility:** `pub` only for the crate's API; `pub(crate)` for the rest.
- **IDs are newtypes** (`TaskId`, `SprintId`, `ProjectId`, …), never bare `String` in APIs.

## Dependencies

- **Ask first.** A new dependency is proposed to the user with the reason and the alternatives before it is added.
- Versions live in `[workspace.dependencies]`; crates use `dep.workspace = true`.
- cargo-deny must pass: licences, advisories, bans.

## Command-line tools

- Every Rust command-line tool, spikes included, parses its arguments with `clap` using the derive API
  (`#[derive(Parser)]`, subcommands as an enum), following the command tree in
  [improving-atm-cli-ergonomics.md](../improving-atm-cli-ergonomics.md).

## Errors

- **Library crates** (atm-lib and the layers under it) define typed error enums with `thiserror`, one per crate or
  per area, with variants callers can act on.
- **Binaries** (`atm`, `atm-serve`, the app's `main`) may use `anyhow` to add context at the top level.
- **No `unwrap`** outside tests. **`expect`** only for a proven invariant, with a message saying why it can't fail,
  e.g. `expect("task IDs are non-empty by construction")`.
- Errors are safe to print to agents and to log: no credentials, no file contents beyond what the user asked for
  ([security-architecture.md](../security-architecture.md)).

## Async

- `common`, `db`, `vcs` and `lib` stay synchronous and do no hidden I/O beyond their interfaces.
- The REST API (`api`) may be async; the runtime and web framework are an open question in
  [porting-to-rust.md](../porting-to-rust.md) §6. Once decided, record it there and follow it.

## Logging

- Libraries emit structured events and spans with `tracing`. Only binaries install a subscriber.
- Never log task text; log IDs, counts and timings.

## Documentation

- Every public item gets a `///` doc comment: what it does, and its arguments where not obvious.
- Public functions that can fail have an `# Errors` section; key APIs have a runnable `# Examples` doctest.
- Each crate's `lib.rs` starts with a `//!` summary pointing to its design documents.
- No other comments unless the code can't say it: a non-obvious reason, an invariant or a link to a decision.

## Tests

- **Unit tests stay with the code they test,** in a `#[cfg(test)] mod tests` at the bottom of its file, while the
  file reads well as a whole.
- **When the tests hurt the code's readability, they move out** to a file of their own, `foo/tests.rs`, declared at
  the bottom of `foo.rs` with `#[cfg(test)] mod tests;`. When that file grows large, `foo/tests/` holds one file per
  group of behaviours.
- **Integration tests** are in the crate's `tests/` directory, one file per area (e.g. `tests/sprints.rs`). An area
  that grows large is a directory, `tests/<area>/main.rs`, with a module per group. The scenarios in
  `tests/scripts/scenarios/` stay the shared check of behaviour: the Rust `atm` passes them as the Python one does.
- **About 400 lines is a guideline for when a file reads badly, not a threshold.** Judge how the tests affect reading
  the code before splitting: 30 lines of code under 200 lines of tests is worth splitting, while 230 lines of code
  with 300 lines of tests may read fine as one file. Test data (fixtures, tables of cases) may be longer, in files of
  its own.
- **Property tests** with `proptest` for the merge driver, the status machine ([task-states.md](../task-states.md))
  and other algebraic code.
- **Benchmarks** use `criterion` and live in the crate's `benches/` directory. Targets are percentiles (e.g. p95),
  which criterion doesn't report: benchmarks record each iteration's time in `iter_custom` and compute the
  percentiles from the measurement-phase samples.
- **Shared fixtures** move into an `atm-test-support` crate once several crates need them.
- Test names say the behaviour: `close_carries_unfinished_tasks`, not `test_close`.
- Rust crates keep at least 99% line coverage.
