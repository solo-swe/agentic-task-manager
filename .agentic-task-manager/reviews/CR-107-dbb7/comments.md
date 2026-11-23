## crates/atm-lib/tests/reviews/lifecycle.rs:11 · round 1 · t1
> fn set_status(root: &std::path::Path, id: &str, status: &str) {

- [x] **Hakan Serce** · 2026-11-21T13:56:00Z · c1
  where is the abstraction here? are we really setting the status of a path, or a task, review? Introduct proper abstractions.
  - **claude:0f56a916** · 2026-11-21T14:01:32Z · c2
    Right, it poked the JSON file by path. The tests now play the events themselves. A fixture `lapse(project, task)` lets the claim run out through the store, the lease put in the past, and the next atm command (`atm claim list`) applies the real lapse. A shared setup, `sent_back_then_lapsed`, has Hakan send CR-1 back, the agent take T-1 back, then its claim lapse. Part 1 file edit (a task gone) is now `drop_task(project, task)` through the store. **Doing it for real found a bug:** a lapse clears `started_by`, so neither a round nor ATM-631 openings knew the to-do task was the author own (the old fixtures kept `started_by`, which no real lapse leaves). `lifecycle::lapsed_for(task, agent)` reads the lapse note ("lease expired (agent)") and serves both. New test: opening a review after a real lapse takes the task back, and another agent lapse does not. atm-lib tests and clippy pass. In round 2.
