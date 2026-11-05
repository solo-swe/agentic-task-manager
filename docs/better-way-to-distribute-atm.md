# Better ways to distribute atm

Currently, atm is a Claude Code plugin that comes with the cli and a dashboard when plugin is installed from the marketplace. Although this approach has been working fine with a small cli and dashboard, as we add new features and change architecture, and also support multiple harnesses the approach is not able to keep up.
In this document we are analyzing this issue and proposing to change the distribution model.

The proposed model can be summarized as follows:

1. Write one skill (set of skills) and use it everywhere.
2. Install the app the normal way, and have the app's own CLI place the skills, with the UI button as a front end to that command.
3. Consider marketplaces (only some harnesses have this) and `npx skills` as extra ways for people to find you, not the main install path.

## 1. One skill, not one per harness

You don't need per-harness skills. The SKILL.md format is shared, and the main difference between harnesses is which directory they read.

- **Stick to the portable core.** Use `name` and `description` in the frontmatter, a markdown body, and `references/` files that the skill links to. Some fields work only in some harnesses. For example, `context: fork` is Claude Code-only per one compatibility matrix. Extra fields are usually ignored elsewhere, but don't let the skill depend on them.
- **Directory locations.** `~/.agents/skills/` is becoming the shared location. Cursor reads `.agents/skills/`, as do Codex CLI, Gemini CLI, and VS Code Copilot. Cursor also picks up `.claude/skills/` and `.codex/skills/`. Claude Code still uses `~/.claude/skills/`, and a few others have their own paths (Copilot, Windsurf, Goose…). So in practice you write the same folder to 2–4 locations.
- **Install user-wide, not per project.** Your app is a tool on the machine, not part of a repo, so the skill should be available in every project.

## 2. The real problem: the skill and the binary drift apart

Once a skill is installed separately from the app, it can describe flags from v1.2 while the user is running v1.5. This matters more than which harness you target. I'd use a two-layer design:

- **A thin, stable SKILL.md:** what the app is, the core concepts, the 5–10 most common workflows, rules ("always pass `--json`", "use `--dry-run` before destructive ops"), and how to get more detail.
- **Version-matched docs from the binary itself:** e.g. `yourapp agent-docs <topic>` prints detailed reference for exactly the installed version. The skill tells the agent to run it when it needs specifics. `--help` covers part of this, but a dedicated agent-oriented output is better.

The skill can also have the agent run `yourapp --version` and state a minimum supported version, so an old binary produces a clear "please upgrade" message rather than confusing errors.

## 3. Plugins: thin wrappers, only where they help

Plugins are harness-specific packages around the same skill. They're worth it when they add something a bare skill can't:

- **Claude Code plugin** (`.claude-plugin/plugin.json` + a marketplace repo): adds slash commands (`/yourapp:status`), hooks, and updates through `/plugin`.
- **Codex plugin**: a manifest-driven bundle that can package three types of component (skills, MCP servers, app connectors) and installs through a marketplace JSON in a Git repo. One limitation: self-serve publishing to the official Codex Plugin Directory is not yet available as of May 2026. That may have changed since, but for now you'd distribute it from your own GitHub repo.
- **Gemini CLI extensions, Cursor, etc.**: only if users ask for them.

Keep everything in **one repo with one copy of the skill**, and point every plugin manifest at it:

```
yourapp-agents/
  skills/yourapp/SKILL.md, references/...
  .claude-plugin/plugin.json       → skills: ../skills
  .claude-plugin/marketplace.json
  .codex-plugin/plugin.json        → skills: ./skills/
  .agents/plugins/marketplace.json
```

Don't use a plugin to install your binary, e.g. from an install hook. It's fragile and surprising for users, and an install hook that downloads executables looks exactly like the malware patterns these ecosystems have been dealing with.

## 4. Distribution: decouple the app from the agent integration

I'd set it up in three layers:

**Layer 1: install the app the normal way.** Homebrew (formula or cask), winget/scoop, `cargo install`/a release script, and a notarized DMG. These handle PATH, updates, and uninstall, which marketplaces can't.

**Layer 2: the app installs its own skills. This is the main path.** Add a CLI command:

```
yourapp agents install [--harness claude,codex,cursor,...|--all] [--project]
yourapp agents status
yourapp agents uninstall
```

- Detect installed harnesses by checking for `~/.claude`, `~/.codex`, `~/.cursor`, `~/.gemini`, and so on, and always write to `~/.agents/skills/` as well.
- Keep the real skill in an app-managed directory (e.g. `~/.local/share/yourapp/skills/yourapp`) and **symlink** it into each harness directory. Then every app upgrade updates the skill automatically, which fixes the drift problem at install time. The popular `npx skills` CLI works this way: it downloads the skills into a central .agents/skills directory, and for every agent you use, copies or symlinks the relevant files into the appropriate folders. Fall back to copying, and re-copy on upgrade, if a harness has trouble with symlinks.
- Make the command idempotent, and have it print what it wrote where. Users of dev tools want to see what changed in their dotfiles.
- **The UI button** ("Connect to Claude Code / Codex / Cursor…", with checkboxes for detected harnesses) just calls this same code. Don't build a second implementation.
- Optionally, print a hint after `brew install` or on first run: "Run `yourapp agents install` to let your AI agents use yourapp". Don't install skills silently. Writing into another tool's config directory should be opt-in.

**Layer 3: listings for discoverability (secondary).**

- Publish the skill repo publicly so `npx skills add you/yourapp-agents` works and the skill shows up on skills.sh.
- List the Claude Code and Codex plugins in your own marketplace repos.
- The first thing a skill installed this way should do is check `command -v yourapp` and, if it's missing, tell the user how to install the app, not try to work around it.

## 5. App Store caveat

If you mean the Mac App Store, its sandbox works against this whole design. A sandboxed app can't freely write to `~/.claude/skills` and similar directories. It would need the user to grant each folder through an open panel. Putting a CLI on PATH is also awkward. For a dev tool with a CLI, a background service, and an agent integration, Homebrew cask plus a notarized direct download is a much better fit. If you still want an App Store listing, treat it as a separate, more limited edition.

## Suggested rollout

1. Thin SKILL.md, plus `yourapp agent-docs` for version-matched detail
2. `yourapp agents install/status/uninstall` using symlinks, with a UI button that calls it
3. A public skills repo, so `npx skills add` works
4. A Claude Code plugin (and a Codex plugin if your users are there) that wraps the same folder
5. Other harness-specific plugins only on demand

Sources:

- [vercel-labs/skills CLI (fork with agent path table)](https://github.com/Harries/skills-cli)
- [add-skill compatibility matrix](https://github.com/ingf/add-skill)
- [Skills on Cursor](https://handsonai.info/platforms/cursor/skills/)
- [Codex CLI plugin system](https://codex.danielvaughan.com/2026/03/30/codex-cli-plugin-system)
- [Codex marketplace distribution](https://codex.danielvaughan.com/2026/04/11/codex-marketplace-plugin-distribution/)
- [Vercel Skills 101](https://levelup.gitconnected.com/vercel-skills-101-88b995b6125d)
