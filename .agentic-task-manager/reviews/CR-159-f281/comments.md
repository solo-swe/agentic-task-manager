## (the review) · round 1 · t1
- [ ] **Hakan Serce** · 2026-11-24T16:30:28Z · c1
  is this specific to atm? It seems there are generic parts to this that can be applicable to other projects as well?
  - **claude:ba530ebb** · 2026-11-24T16:31:30Z · c2
    Yes, most of it is generic. It splits in two:

    **Generic: the hook.** Refusing compound commands, interpreter one-liners, `sed -i`, here-documents, `gh --jq` and quoted shell syntax, with what to run instead, is about how Claude Code's permission check reads a command, not about this repo. Every project an agent works in gains from it. The one atm-flavoured part, letting `atm claim hold <id> && <one command>` through, applies to every project that uses atm's skills, not just this one.

    **This repo's: the allow rules.** `pnpm --dir dashboard …` and `node --test tests/scripts/*` name this repo's layout and tools. They belong in its `.claude/settings.json`.

    **Where the hook could live** (my recommendation first):
    1. **Shipped with atm, installed by `atm agents hooks install`.** That command already adds atm's review hook to the user's `~/.claude/settings.json` (or a project's), only on the user's command and only its own entries (I35). It would add a `PreToolUse` entry on `Bash` running atm itself (say `atm _hook bash`), with the check in Rust in atm-lib, unit-tested there. There's no Node script to install, it works in every project, and it's on for everyone who installs atm's hooks. The docs (`atm docs`) would say what it refuses and why.
    2. A script in your user settings (`~/.claude/hooks/`): generic but by hand, and outside any repo, so neither versioned nor tested.
    3. This repo only, as proposed: the narrowest, and the other projects don't gain.

    If you take 1, I'd make it its own task (atm-lib's check with its tests, the hook in `atm agents hooks`, the docs, the security doc's I35). This task would then add only this repo's allow rules on your yes. Which do you want?
