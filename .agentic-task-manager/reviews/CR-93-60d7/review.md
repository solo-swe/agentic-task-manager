+++
id = "01a1fac0-4cf7-7780-a07d-31c8d54460d7"
number = 93
title = "ATM_NOW in debug builds only; lease length, timeouts, times and task numbers bounded"
status = "approved"
tasks = ["ATM-666"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T20:57:49Z
updated = 2026-11-20T22:38:52Z
+++

Audit findings C4, C5, C16 and L9: the clock, lease lengths, timeouts and task numbers.

**C4, the clock.** `ATM_NOW` replaced the time in every build, so `ATM_NOW=2099-… atm loop next --claim` let anyone take every other agent's task, with all their leases run out by that clock. `ATM_NOW` now works in a debug build only (`time::CLOCK_OVERRIDE = cfg!(debug_assertions)`). Every binary the tests run is a debug `cargo build`, and releases are `--release`.
- *Not as the task says:* the task offered a second, test-only variable as the alternative. I didn't use it: whoever can set one variable can set two, so it would be no barrier.
- *A test the task asks for that I didn't write:* "a lease stays live under `ATM_NOW` alone" can't be a CLI scenario, because the CLI suite's binary is a debug build, where `ATM_NOW` works by design. A unit test pins the rule instead.

**L9, the lease length.** `lease_minutes` is at most a year, and `inf`, `NaN` or `-inf` fall back to the default. Before, `1e300` overflowed a lease's seconds, so no claim ever ran out. `lease_until` and `offered_to` use `saturating_add`.

**C5, the timeout.** `--timeout 1e20` panicked. It is now refused as a usage error (exit 2), through `Duration::try_from_secs_f64`.

**C16, times and numbers.**
- `parse_iso` refuses seconds 60 and 61. Before, `23:59:60` was the next day's midnight written another way.
- `next_task_number` refuses to go past the largest number there is instead of wrapping round to one already given, and `task add` checks that the last of its new numbers fits too.

**Tests**
- Unit:
  - the clock rule;
  - `lease_minutes` with `1e300`, `"1e300"`, two years, `inf` and `NaN`, and a lease taken with `1e300` running out within a year;
  - `parse_iso` with seconds 59, 60 and 61;
  - `next_task_number` at `u64::MAX`;
  - `--timeout 1e20` and `1e20h`.
- `docs/security-architecture.md`: a new invariant, I34.
- Passing: atm-common's, atm-lib's and atm-cli's tests, the CLI suite, and clippy.
