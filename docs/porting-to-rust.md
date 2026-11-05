# Porting `atm-cli` to Rust

**Status:** proposal — not yet implemented. Depends on the Python modernization tasks (T-412, T-414).

This document explores replacing the Python `atm-cli` implementation with a Rust implementation, producing a single static executable for customers while preserving the `atm` command interface.

---

## 1. The case for Rust

Today `atm-cli` is pure Python standard library, bundled into `plugin/scripts/atm.py` by `atm-cli/build.py`. Customers need only `python3 >= 3.9`. Developers have no third-party dependencies (by design).

Rust changes the tradeoffs:

| Benefit | What it means for `atm` |
|---------|------------------------|
| **Single static binary** | Customers need nothing — not even Python 3.9. Even better than the current "one `.py` file" story. |
| **Compile-time checks** | Type errors, enum exhaustiveness (task states!), and borrow-checker safety catch bugs before users run it. |
| **`cargo` ecosystem** | `clap` for CLI, `serde` for JSON, `axum`/`actix` or std HTTP for the server — no hand-rolling parsers. |
| **Performance** | Negligible for most CLI commands, but welcome for the JSON merge driver and large `tasks.json` files. |
| **Existing skill** | You already use Rust productively in other projects. |

---

## 2. What you give up (ATM-specific costs)

### A. The "readable script" ethos
`plugin/scripts/atm.py` is a committed, human-readable file. Users can inspect what the plugin runs before executing it. AGENTS.md and `docs/helper-runtime-proposal.md` both emphasize this transparency. A compiled Rust binary is opaque. You lose that unless you commit source and require users to build — which violates the "no build step for customers" rule.

### B. Cross-platform distribution
Today the "build" is `cat modules → atm.py`, running wherever Python 3.9 runs (macOS, Linux, WSL). With Rust you must compile and ship binaries for:
- macOS x86_64 + Apple Silicon (arm64)
- Linux x86_64 (+ arm64?)
- Windows

That's a CI matrix, code signing considerations, checksums, and release machinery — entirely new infrastructure for a project that currently has none.

### C. The git merge driver
`atm merge-driver` runs inside `git merge`'s environment. A Rust binary works fine, but it must be on `PATH` and behave identically across platforms. Currently one `atm.py` covers both CLI and merge driver. With Rust you'd still ship one binary, but you'd need to ensure it's registered correctly in `.gitattributes` / git config on every checkout.

### D. `atm serve` / REST API
The dashboard server is a separate process (`plugin/scripts/atm-serve`). Rust handles this well (Axum, Actix, or std HTTP), but it's another binary to ship and version alongside the CLI.

---

## 3. What the Rust port would look like (sketch)

```rust
// atm-cli/src/lib/
mod common;    // vocabularies, errors, model
mod db;        // store interface + jsongit implementation
mod vcs;       // git interface
mod lib;       // project, loop, tasks, status (state machine here)
mod cli;       // clap-based subcommands
mod api;       // REST server (axum or std)
```

Rust enums make the task-state machine far safer than the current Python dict-based dispatch:

```rust
// lib/status.rs — compile-time exhaustiveness
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskState { Todo, Active, Review, Blocked, Done, Skipped }

impl TaskState {
    fn allowed_transitions(&self) -> Vec<TaskState> { ... }
    fn can_transition(&self, to: TaskState) -> bool { ... }
}
```

Invalid transitions become compile errors rather than runtime `AtmError`s. The `STATUS_CHANGES` table in `atm/common/model.py` becomes a Rust `match` or transition table checked at compile time.

### CLI with `clap`

```rust
// cli/task.rs
use clap::{Args, Subcommand};

#[derive(Subcommand)]
enum TaskCmd {
    List { .. },
    Add { .. },
    Edit { id: String, .. },
    State {
        id: String,
        #[arg(value_enum)]
        state: TaskState,
        note: Option<String>,
    },
}
```

---

## 4. Sequence: improve Python first, then port

**Do not port to Rust right now.** The CLI redesign (`docs/improving-atm-cli-ergonomics.md`) and build modernization (`docs/modernizing-build-approach.md`) are still proposals. Porting a work-in-progress means designing the CLI twice.

### Recommended sequence

```
Phase 1: Python — design & stabilize
  └─ Decide Click vs Typer
  ├─ Rewrite command groups with nested subcommands
  ├─ Replace hand-rolled status machine with a state machine
  ├─ Freeze the command structure and behavior
  └─ (T-412, T-414)

Phase 2: Rust port
  └─ Translate the now-stable Python spec to Rust
  ├─ clap for CLI (mirrors the Click/Typer design)
  ├─ serde + enums for task states (compile-time exhaustiveness)
  ├─ Cross-compile CI matrix for macOS/Linux/Windows
  └─ Ship single static binary; keep `atm.sh` launcher as a thin wrapper
```

### Exception: port now only if
You freeze the Python CLI design as-is and treat the Rust port as a reimplementation of current behavior, with the ergonomics redesign happening natively in Rust afterward. That's valid, but it means two design phases regardless.

---

## 5. Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Losing readable-script transparency | High | Medium | Commit Rust source; document that binary is built from it |
| Cross-platform CI burden | High | High | Start with macOS + Linux; add Windows later |
| Merge driver registration breaks | Medium | High | Test `atm merge-driver` in CI on all platforms |
| `atm serve` parity | Medium | Medium | Keep Python `atm-serve` as fallback during transition |
| Dependency drift in `cargo` | Medium | Low | Pin versions in `Cargo.lock`; vendor if needed |

---

## 6. Open questions

- Should the Rust port be a full replacement, or should Python remain as a fallback for platforms without a Rust binary?
- Which Rust web framework for `atm serve` (Axum, Actix, std HTTP)?
- Should the binary be distributed via GitHub Releases, or built by the user?
- How to handle the `atm.sh` launcher — thin wrapper around the binary, or replaced entirely?
- What minimum Rust version (MSRV) to target?

---

## 7. Related documents

- `docs/improving-atm-cli-ergonomics.md` — proposed `atm` command structure (the spec to port)
- `docs/modernizing-build-approach.md` — developer stack, Click/Typer, build evolution
- `docs/task-states.md` — current task state model and transitions
- `docs/helper-runtime-proposal.md` — original Python-only runtime decision (B)
- `docs/layered-architecture.md` — layer rules to preserve in the port
