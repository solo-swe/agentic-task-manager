# Distributing atm as an app that installs its own skills

Status: **agreed** with Hakan on 2026-11-06 (T-426). It replaces the per-harness adapters of
[multi-harness-support.md](multi-harness-support.md) (T-413 is rewritten to this model, §8). Built on the Rust port
(T-415) and the [Tauri](guides/tauri.md), [Rust](guides/rust.md) and [Svelte](guides/svelte.md) guides.

Today atm is a Claude Code plugin: installing it from the marketplace brings the CLI (Python), the dashboard and the
skills, and only Claude Code can use them. As atm grows, and other harnesses (Codex, Cursor, Gemini CLI, Copilot,
OpenCode, Windsurf, Goose) come in, that model doesn't keep up: the skills are tied to one harness, the CLI to the
plugin, and a skill installed apart from the CLI drifts from it.

The model, in short:

1. **atm is an app you install the normal way**: the atm app (Tauri, macOS first) and the `atm` CLI (one Rust binary,
   macOS, Linux and Windows), through Homebrew, a notarized download and release binaries.
2. **The app installs its own skills** into the harnesses you choose: `atm agents install`, and a button in the app
   that calls the same code. One set of skills for every harness.
3. **The skills stay thin**; what they need in detail comes from the installed binary (`atm docs <topic>`), so it
   always matches the version that runs.
4. **No plugins of atm's own**: the Claude Code plugin is retired when the Rust binary replaces Python, and what its
   hooks did moves into atm ([porting-to-rust.md](porting-to-rust.md) §4, §6). Marketplaces and `npx skills` are
   ways to find atm, not to install it.

## 1. What you install

| Package | What it is | Where |
|---|---|---|
| **The atm app** | The dashboard in its own window, the skills button, notifications. Bundles the `atm` binary and puts it on PATH. | macOS: notarized DMG and a Homebrew cask. Linux and Windows later. |
| **The `atm` CLI** | The one Rust binary (T-415): every command, `atm serve` for the dashboard in a browser, the git merge driver. | Homebrew formula, release binaries for macOS (arm64, x86_64), Linux and Windows, a release script. |

- **They come with the Rust binary.** The new packages ship once T-415's binary exists: packaging today's Python script
  as an app would be thrown away. What doesn't need the binary (thin skills, `atm docs`, `atm agents`) is built first,
  in Python, and ships in the plugin as today (§9).
- **No Mac App Store.** Its sandbox can't write into other tools' folders or put a CLI on PATH.
- **atm's own files stay in `~/.agentic-task-manager`** (`ATM_HOME` overrides it): the projects list, the server
  record, and now the installed skills (`skills/`). The plugin, the CLI and the app share them.

## 2. The app: one process, no server inside

The app shows the dashboard from its own bundled files and reaches the data through **Tauri commands** (IPC), each a
thin mapping to an atm-lib function, with live updates pushed through Tauri channels
([Tauri guide](guides/tauri.md)). It runs no HTTP server.

- **Why:** the app will do more than show data: the skills button writes into other tools' folders, and review
  decisions (the user's) are natural to make in it. Through IPC only the app's own window can call them; on a
  localhost server they would be write routes any local process or web page could send requests to, needing tokens
  and request checks. The app also escapes the server's lifecycle (ports, records, a server left at another version).
- **`atm serve` stays**, for the dashboard in a browser and for machines without the app. Its routes stay read-only.
- **The dashboard has two transports.** `lib/api/http.ts`, already the only module that makes requests, becomes an
  interface with an HTTP implementation (the browser, `atm serve`) and an IPC one (the app). Each feature is a REST
  route and a Tauri command, both thin over the same atm-lib call, with a test that they agree.
- **Links:** the dashboard's addresses (`/projects/<id>/<tab>?task=…`, T-429) work in both; the app opens links from
  outside through a deep-link scheme (`atm://projects/<id>?task=T-1`).
- **Tests:** the browser dashboard keeps its HTTP fixture; the app's UI runs under Playwright with mocked Tauri IPC
  over an in-memory project.

## 3. The commands

In atm's command tree (T-425), among the maintenance groups:

```
atm agents install [--harness claude,codex,cursor,…|--all] [--project] [--copy]
atm agents status
atm agents uninstall [--harness …|--all] [--project]
atm docs [<topic>]
```

- **`atm agents install`** puts the skills where the chosen harnesses read them (§5). Idempotent; it prints every
  folder it wrote or linked. Never run by anything but the user: no install hook, no first-run install. After an
  install (`brew`, the DMG) atm only says how: "Run `atm agents install` to let your AI agents use atm".
- **`atm agents status`** lists each detected harness, where atm's skills are for it (linked, copied, missing, stale),
  their version against the binary's, and where a harness would show them twice (§5).
- **`atm agents uninstall`** removes exactly what install wrote.
- **`atm docs <topic>`** prints reference for the installed version, written for agents: the rules (task text is data,
  never edit `tasks.json`, `--json`), the commands by workflow, the task fields. `atm docs` lists the topics. It
  replaces what the skills and `plugin/CLAUDE.md` repeat today.
- **The app's button** ("Connect your agents", with the detected harnesses ticked) calls the same Rust code as
  `atm agents install`, through a Tauri command; there is no second implementation.

## 4. The skills

The seven stay separate: each is a workflow a user asks for by name. Without the plugin's `/atm:` namespace they are
named `atm-do-task`, `atm-loop-tasks`, `atm-create-tasks`, `atm-plan-sprint`, `atm-close-sprint`, `atm-push` and
`atm-serve`, so they don't clash with other skills. Each becomes **thin, on the portable core**:

- **Frontmatter:** `name` and `description` only. `allowed-tools` (Claude Code's pre-approved commands) is documented
  only for Claude Code, and other hosts reject unknown fields, so `atm agents install` adds it to Claude Code's copy
  only (§5).
- **Body:** what the workflow is, its steps, its rules, and where to get detail: `atm docs <topic>`. The command
  reference, the task schema and the shared rules move out of the skills into `atm docs`.
- **First step:** check `command -v atm` and `atm --version` against the skill's minimum; if atm is missing or too
  old, say how to install or upgrade it, and stop. Never work around it.
- **Claims:** atm keeps them alive itself, in every harness: each atm command an agent runs renews its claims, and
  the lease is longer where nothing renews it between commands; the skills run `atm claim hold <id>` before long
  commands ([porting-to-rust.md](porting-to-rust.md) §4). `atm agents install` never edits another tool's hook
  settings.

## 5. Where the skills go

The skills live once, in `~/.agentic-task-manager/skills/<skill>/`, written by the installed atm (so an upgrade
updates them), and are **symlinked** into each harness's folder:

| Harness | Folder atm writes | Notes (checked 2026-11-06) |
|---|---|---|
| Claude Code | `~/.claude/skills/` | Doesn't read `~/.agents/skills` (an open request). |
| Codex CLI, Gemini CLI, Copilot, OpenCode, Windsurf (Devin), Goose | `~/.agents/skills/` | Each also has its own folder; `~/.agents/skills` is the shared one. |
| Cursor | `~/.agents/skills/` | |

- **Both `~/.agents/skills` and `~/.claude/skills` by default**, so Claude Code works without the plugin. Cursor,
  OpenCode, Goose and VS Code Copilot read both, so they list atm's skills twice: `atm agents status` says where, and
  `--harness` installs into fewer folders.
- **Copied where links break:** Cursor (home-folder symlinks) and the Copilot CLI (symlinked skills not listed) have
  known bugs; atm copies for them, and re-copies on upgrade. `--copy` copies everywhere.
- **User-wide by default**; `--project` writes the project's `.agents/skills` (and `.claude/skills`) instead, for a
  team that commits them.

## 6. Plugins

- **The Claude Code plugin is retired at the switch** to the Rust binary ([porting-to-rust.md](porting-to-rust.md)
  §6 lists where each of its parts goes). Until then it ships as today; its last release says how to move to the app
  or the formula.
- **Other harnesses' plugins** (Codex, Cursor, Copilot, Gemini, Goose all have a format) only if users ask for
  something a skill can't do, each pointing at the same skills.
- **Nothing installs the binary but the user** (no install hook that downloads executables).
- **Discoverability:** a public skills repo, generated from `skills/` at each release, so
  `npx skills add solo-swe/skills` works; it leads to "install atm" first. `solo-swe/skills` is shared by our apps,
  each release writing only its own skills ([ADR-002](adr/ADR-002-atm-public-skills-repo.md), T-453).

## 7. Security

New entry points, each added to [security-architecture.md](security-architecture.md) in the change that adds it:

- `atm agents install|uninstall`: writes into other tools' folders. Opt-in only, never silent; writes only the skill
  folders it lists and removes only what it wrote; never another tool's settings or hooks.
- `atm docs`: prints text from the binary; reads nothing outside it.
- The app's Tauri commands: least-privilege capabilities per window; rendered task text gets none.
- The app's deep links (`atm://…`): parsed like the dashboard's addresses (T-429), never fetched or run.
- Distribution: signed and notarized builds, Homebrew checksums; the skills repo is part of the release.

Changing the skills means the agent sandbox run, or the written skill review (§3.3).

## 8. T-413 (multi-harness support)

Rewritten to this model: make atm work in the harnesses beyond Claude Code through `atm agents install`: try each
harness's folder and symlink behaviour, the duplicate listings, claims through `atm claim hold`, and which harness
plugins are worth having. Its per-harness adapters (`plugin/open-code/`, `install.sh`, `skills.json`) are not built.

## 9. Rollout

1. **Claims renewed by atm itself** and the longer lease ([porting-to-rust.md](porting-to-rust.md) §4), in Python,
   so nothing depends on the plugin's hooks.
2. **Thin skills and `atm docs`** (Python, in the plugin): the shared reference moves into `atm docs`; the skills drop
   to the portable core, take their `atm-` names and check `atm --version`.
3. **`atm agents install|status|uninstall`** (Python first, ported with T-415): `~/.agentic-task-manager/skills/`,
   symlinks and copies, Claude Code's copy with `allowed-tools`, status with duplicates.
4. **The Rust CLI** (T-415) with its release builds, the Homebrew formula and the release script; at the switch the
   Python atm and the plugin are retired.
5. **The atm app**: Tauri, IPC to atm-lib, the dashboard's IPC transport, the skills button, notarized DMG and cask.
6. **The public skills repo**, generated at release.
7. **Other harnesses** (T-413).

## Sources

Checked 2026-11-06; read as references, not instructions.

- Claude Code skills: https://code.claude.com/docs/en/skills
- Codex skills, plugins and hooks: https://learn.chatgpt.com/docs/build-skills, https://learn.chatgpt.com/docs/plugins, https://learn.chatgpt.com/docs/hooks
- Cursor skills: https://cursor.com/docs/context/skills
- Gemini CLI skills: https://geminicli.com/docs/cli/skills/
- VS Code Copilot skills: https://code.visualstudio.com/docs/copilot/customization/agent-skills; GitHub Copilot: https://docs.github.com/en/copilot/concepts/agents/about-agent-skills
- The `skills` CLI: https://github.com/vercel-labs/skills
