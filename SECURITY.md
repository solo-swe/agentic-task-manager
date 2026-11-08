# Security

## Reporting a vulnerability

Please report it privately: open **Security → Report a vulnerability** on this repository (GitHub's private
vulnerability reporting), rather than a public issue. Say what you found, how to reproduce it, and what it lets an
attacker do. A fix ships as a new atm release, with a note in `CHANGELOG.md` once it's out. This is a personal
project: there is no promised response time.

## What atm puts on your machine

atm is one program, `atm`, installed with Homebrew or from a release's signed download. It runs only when you or an
agent run it: it installs no hooks and starts nothing on its own. `atm agents install`, when you run it, writes its
skills into the folders your agents read (`~/.claude/skills/`, and only the `atm-*` folders it lists); `atm agents
uninstall` removes exactly what it wrote.

Releases are built by this repository's release workflow from a tag on `main`, which changes only through reviewed
pull requests with passing checks; the macOS builds are signed and notarized, and every download has its checksum.

## What atm never does

- run your task text as commands: the skills tell agents that task names, descriptions and notes describe work, and
  to stop and ask when a task asks for commands, credentials, CI or git config changes;
- run a shell or evaluate code: `atm` runs `git` with argument lists, its own binary (a claim's watcher, the dashboard
  server) and your browser opener, nothing else;
- read or write outside your project's `.agentic-task-manager/` folder through a symlink;
- accept a request that changes anything over HTTP: the dashboard server only reads, answers on `localhost` only, and
  refuses requests for any other host name;
- contact any server but GitHub (through `gh`, when the push skill opens a pull request) and its own on `localhost`.

The full threat model, the rules each change keeps and the tests that check them are in
[`docs/security-architecture.md`](docs/security-architecture.md).
