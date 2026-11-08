# Claims: which agent works on which task, and how long

- **A claim** says which agent works on a task: `atm loop next --claim` and `atm task state <id> in_progress` take one,
  every other change of status drops it. Only its holder may change the task's status; another agent gets exit code 4
  ("claimed by … until …"). Claims are shared by every git worktree of a clone.
- **The lease:** a claim lasts two hours by default (`meta.lease_minutes` sets a project's own). Every atm command you
  run renews your claims (once their last renewal is ten minutes old), so you don't need to; `atm claim heartbeat <id>`
  renews one by hand.
- **Long commands** (a full test suite, a build, waiting on CI) run no atm command for a while: start them after
  `atm claim hold <id>`, in the same shell (`atm claim hold <id> && <command>`). It runs nothing: a watcher renews the
  claim while that shell lives (four hours at most).
- **If a lease runs out,** the task goes back to `todo` for others, with a note, its commit kept. The next command you
  run on it says so (exit code 4) and how to go on: `atm task state <id> in_progress`, then your command again.
- **Who you are:** atm takes the actor from `--agent`, else `ATM_AGENT`, else the agent session, else git's user name.
  Never pass `--agent` to act as a person: a review mode or a decision depends on who acts.
