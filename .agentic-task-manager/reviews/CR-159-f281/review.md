+++
id = "01a20e26-83e8-72ff-aad1-94e124fff281"
number = 159
title = "Proposal: Claude Code settings that keep agents from tripping permission prompts"
status = "open"
tasks = ["ATM-746"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T15:22:14Z
updated = 2026-11-24T16:31:30Z
+++

**A proposal, not applied** (ATM-746): permission settings are yours to change. This review is of `.claude/settings.json` as it is; the change below goes in on your yes, as one commit: the settings and `.claude/hooks/plain-bash.mjs`.

## 1. Allow rules: the forms agents run, read-only or test-only

As exact as the existing ones: no `Bash(node *)`, no `Bash(pnpm run *)`.

```json
"Bash(pnpm --dir dashboard test)",
"Bash(pnpm --dir dashboard test *)",
"Bash(pnpm --dir dashboard run check)",
"Bash(pnpm --dir dashboard run build)",
"Bash(pnpm --dir dashboard run build:page)",
"Bash(pnpm --dir dashboard run test:e2e)",
"Bash(pnpm --dir dashboard exec vitest *)",
"Bash(pnpm --dir dashboard exec playwright test *)",
"Bash(pnpm --dir dashboard exec prettier *)",
"Bash(node --test tests/scripts/*)"
```

`prettier *` can `--write`, as the existing `pnpm exec prettier *` can: formatting only. Optional cleanup: drop `Bash(crit comments *)`, since Crit is gone since 0.40.

## 2. A PreToolUse hook on Bash: refuse at once what would prompt

```json
"hooks": {
  "PreToolUse": [
    {
      "matcher": "Bash",
      "hooks": [{ "type": "command", "command": "node \"$CLAUDE_PROJECT_DIR\"/.claude/hooks/plain-bash.mjs" }]
    }
  ]
}
```

It only refuses: exit 2, with what to run instead on stderr, which Claude Code hands to the agent. Everything else goes on to the permission rules as now.

It refuses:
- **More than one command:** `&&`, `||`, `;`, a pipe, a redirect, `$(…)` or backticks, `VAR=x` in front, `cd`. The one exception is `atm claim hold <id> && <one command>`, how the skills keep a task's lease over a long command.
- **Interpreter one-liners and in-place edits:** `node -e`, `python -c`, `perl -0pi -e`, `sed -i`, a here-document. It points to Read, Edit and Write.
- **`gh … --jq`:** it says to run the plain command.
- **A quoted pattern holding `|` or a backtick:** the permission check reads it as syntax. It points to grep's `-e` per alternative.

Quoted text without syntax goes through, such as a note with `;` or `&`, or `git commit -m 'a > b'`.

<details><summary>plain-bash.mjs</summary>

```js
#!/usr/bin/env node
// A PreToolUse hook on Bash (ATM-746): a command that would stop on a permission prompt is refused at once, with what
// to run instead. It only refuses: what it lets through goes on to the permission rules. Exit 2 with the reason on
// stderr refuses the call and hands the reason to the agent; exit 0 lets it through.
import { readFileSync } from 'node:fs';

/** The command outside its quotes, each quoted part replaced by Q, and whether a quoted part held | or a backtick */
export function unquoted(command) {
  let out = '';
  let quotedSyntax = false;
  for (let i = 0; i < command.length; i++) {
    const c = command[i];
    if (c === "'" || c === '"') {
      const end = command.indexOf(c, i + 1);
      if (end < 0) return { out: out + command.slice(i), quotedSyntax };
      if (/[|`]|\$\(/.test(command.slice(i + 1, end))) quotedSyntax = true;
      out += 'Q';
      i = end;
    } else if (c === '\\') {
      out += 'E';
      i++;
    } else out += c;
  }
  return { out, quotedSyntax };
}

const PLAIN = 'One plain command per call: no &&, ;, |, redirects, $(…) or VAR=x.';
const FILES = 'Read, Edit and Write the file instead (or a test in the repo for a check).';

/** Why `command` is refused, or null when it may go on to the permission rules */
export function refusal(command) {
  const text = command.trim();
  // A long command keeping its task's lease: the hold, then one plain command
  const held = /^atm claim hold [A-Za-z0-9-]+ && (.+)$/s.exec(text);
  if (held) return refusal(held[1]);
  const { out, quotedSyntax } = unquoted(text);
  if (/<<-?\s*\S/.test(out)) return `A here-document writes a file through the shell. ${FILES}`;
  // A one-letter flag cluster (not a --long option) that runs code given inline: -e, -c, -p, perl's -pi and -0pi
  if (/\b(node|python3?|perl|ruby|deno|bun)\b.*\s-(?!-)[a-zA-Z0-9]*[ecpiE]/.test(out))
    return `An interpreter one-liner. ${FILES}`;
  if (/\bsed\s+(-[a-zA-Z]*i|--in-place)/.test(out)) return `sed -i edits a file in place. ${FILES}`;
  if (/(&&|\|\||;|\||>|<|\$\(|`)/.test(out))
    return `${PLAIN} Run each as its own call; to look through output, write it to a file in the scratchpad with a script run by sh, then Read it.`;
  if (/^[A-Za-z_][A-Za-z0-9_]*=/.test(out)) return `${PLAIN} An environment variable goes in a script run by sh.`;
  if (/^cd\s/.test(out)) return `${PLAIN} Use absolute paths, or the tool's own option (pnpm --dir, git -C).`;
  if (/^gh\s.*\s--jq\b/.test(out)) return 'gh with --jq prompts. Run the plain gh command and read its JSON.';
  if (quotedSyntax)
    return 'A quoted pattern holding | or a backtick reads as shell syntax to the permission check. Give each alternative its own -e (grep -n -e one -e two file).';
  return null;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const input = JSON.parse(readFileSync(0, 'utf8') || '{}');
  const why = input.tool_name === 'Bash' ? refusal(String(input.tool_input?.command ?? '')) : null;
  if (why) {
    process.stderr.write(`Refused before a permission prompt (ATM-746): ${why}\n`);
    process.exit(2);
  }
  process.exit(0);
}
```
</details>

**Pipe-tested.** Each case was fed as Claude Code sends it (`{"tool_name":"Bash","tool_input":{"command":…}}` on stdin): 29 cases, all as expected.
- **Through (exit 0):** `pnpm --dir dashboard test`, `… exec vitest run …`, `… exec playwright test …`, `node tests/scripts/run.mjs inbox`, `node --test tests/scripts/cli.test.mjs`, `grep -n project_tasks docs/x.md`, `grep -n -e "one two" -e three …`, `atm claim hold ATM-746 && pnpm --dir dashboard run build`, `atm task note … "a; b & c"`, `git log …..HEAD -i --grep=claude`, plain `gh api …`, `sh script.sh`, `git commit -m 'a > b'`.
- **Refused (exit 2), each with its message:** `&&`, `;`, `|`, `> out 2>&1`, `RUST_LOG=x cargo test`, `cd dashboard && …`, `$(date)`, a hold followed by two commands, `python -c`, `node -e …writeFileSync…`, `perl -0pi -e`, `sed -i`, `cat >> file <<EOF`, `gh api … --jq`, `grep -E "a|b"`, a pattern with backticks.

## 3. Kept as it is

- **The deny list** (cat, head, tail, sed, awk, cd): the hook's messages point to the same tools.
- **Paths outside the project** (`ls ~/.claude`, `curl -o /dev/null …`) still prompt, on purpose: reaching `~/.claude` stays your `/add-dir`.

## 4. Not settings

The auto-mode classifier refused `atm task review approve` (oversight) and `gh secret delete` (destructive), even on your word. No allow rule changes that. Those stay yours to run, or to approve when asked.

## On your yes

I write the two files, try each new allow rule once in a fresh session, and check the hook with one compound and one plain command there. Then I commit and mark the task done.
