# A fresh agent session per task: the evaluation (ATM-740, step 1)

**Status:** findings, for Hakan's decision. ATM-740 asks whether `/atm-loop-tasks`, one long session carrying every
finished task's context into the next, should give way to a runner that starts a fresh headless session per task.
This records what one long session cost per task, what a fresh one would have lacked, and a conflict with a security
invariant that decides how a runner could be built at all (§3).

## 1. What a long session costs (measured)

This session's own transcript (Claude Code's `~/.claude/projects/…/<session>.jsonl`, each assistant message with its
usage), split at the times each task was claimed. "Read from cache" is the context each turn carried, summed over the
turns; "context at most" is the largest one turn carried.

| Window | Turns | Tool calls | Input read from cache | Written to cache | Output | Context at most |
|---|---|---|---|---|---|---|
| closing and planning the sprint | 24 | 27 | 1.7 M | 56 k | 6 k | 85 k |
| ATM-728, three designs with Hakan, 16 tasks | 92 | 162 | 16.3 M | 160 k | 65 k | 251 k |
| ATM-730, a review tab keeps its scroll | 31 | 52 | 8.9 M | 58 k | 17 k | 309 k |
| ATM-732, an unsent comment survives updates | 35 | 67 | 12.1 M | 70 k | 27 k | 379 k |
| ATM-734, the memory analysis | 49 | 80 | 20.7 M | 82 k | 30 k | 461 k |

**The input dominates, and it is the carried context.** ATM-730 grew the context by 58 k tokens of its own, but each
of its 31 turns re-read the 250 k the session had gathered before it: 8.9 M tokens read. A fresh session starts at
about 40 k (the system prompt, `AGENTS.md`, the memory index, the skill); the same task in one would have carried
40 k to about 100 k, about **2.2 M** read: four times less. ATM-734, later in the session, would have been about
**4 M** instead of 20.7 M: five times less. The later in a long session a task comes, the more it costs, whatever the
task: the cost of a task in a loop grows with the number of tasks before it.

(Cache reads are cheaper per token than fresh input, but they are most of what the session spends, and the usage
limits count them.)

## 2. What a fresh session would have lacked

Going through what each of these tasks used from the session rather than from the repository:

| Task | Used from earlier in the session | In the repository for a fresh session? |
|---|---|---|
| ATM-730 | how the e2e fixture serves reviews; that the dashboard was built | yes: `reviews.spec.ts`, `AGENTS.md`; a rebuild costs a minute |
| ATM-732 | the same, and ATM-730's reading of `ReviewPage.svelte` | yes: a few more file reads |
| ATM-734 | nothing task-specific | yes |
| ATM-751–766 (next) | the sidebar decisions made with Hakan | yes: CR-135–137's decisions are recorded in the three design documents (§7 of each), as the reviews required |
| any | an open question asked in chat and not answered (here: move the sprint to `dev/0.44.0`?) | **no**: a question only in chat is lost to the next session |

So these tasks lacked nothing a fresh session couldn't read again for a few tool calls, **because** this project
already writes decisions down: designs are reviewed and record their decisions, tasks link their documents, a task's
notes carry its handoff. What a fresh session lacks is what was **only said in chat**: a question to the user not yet
answered, a "do it this way" not written into a task or document. The rule that makes fresh sessions safe is the one
the skills already have for blocks: a question goes into `atm task state … blocked --question`, not only into the chat.

**Not measured:** doing tasks by hand in fresh sessions (`/clear`, then `/atm-do-task <id>`), as the task's step 1
suggests. A session can't clear itself or start another: that comparison is yours to run (§4, question 2). The
estimate above is from where the context starts and how much each task added.

## 3. The conflict: atm runs no command (I24)

The runner as ATM-740 describes it (`atm loop run --agent '<command template>'`) is the command T-437 removed. I24 in
`docs/security-architecture.md`: **atm runs no command it is given**: `atm loop run <id> -- <command>` was removed
because one permission rule (`Bash(atm *)`) let any command through it, and `cli.test.mjs` checks it stays gone. A
template "only from the user's config" narrows that, but `atm` would again start a program named by data (a config
file the repository could ship, an environment variable), and a pre-approved `atm loop …` would run it.

Three ways to have fresh sessions per task:

| | How | I24 | Who runs the agent |
|---|---|---|---|
| **A. A loop outside atm** (recommended) | atm gives the next task (`atm loop next --claim --json`, or a new `--key` printing only the key) and its outcome (`atm task details --json`); the loop is the user's shell, an example in `docs/agent/` per harness: `while key=$(atm loop next --claim --key); do claude -p "/atm-do-task $key"; done` | kept | the user's shell |
| B. `atm loop run`, the template from the user's own settings only | as ATM-740 describes, with a new ADR replacing T-437's decision | replaced: atm starts one program, from `~/.config` only, never the repository | atm |
| C. The harness's own | Claude Code's headless mode and its own scheduling (`claude -p` per task, a hook at `Stop`) | kept | the harness |

A keeps every invariant and every harness integration as it is (nothing harness-specific in atm, the skills
unchanged), and does what ATM-740 wants: a fresh main agent per task, same model, `AGENTS.md`, memory and skills.
What it can't do is the review watch's hand-over (comments arriving while no session runs): those wait for the next
task's session, whose first `atm loop next` hands back tasks with review decisions first, as it does today.

## 4. Questions for Hakan

1. **Which way:** A, a loop outside atm, documented per harness, with `atm loop next --key` (recommended); B, a new
   ADR bringing `atm loop run` back with its template from the user's settings only; or C, nothing in atm?
2. **The hand comparison:** worth running before deciding? Do two tasks of this sprint each in a fresh session
   (`/clear`, then `/atm-do-task ATM-7xx`), and I compare their transcripts with this table.
3. **If A:** is a fresh session per task the default way to loop in this project (the loop skill pointing to it),
   or one option beside `/atm-loop-tasks`?
