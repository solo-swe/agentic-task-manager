# ADR-002: atm's public skills repo

- **Status:** accepted, 2026-11-08
- **Decided by:** Hakan Serce
- **Context:** T-453, [better-way-to-distribute-atm.md](../better-way-to-distribute-atm.md) §6 and §9 step 6,
  [porting-to-rust.md](../porting-to-rust.md) §2

## Problem

atm's skills have one source, `skills/atm-*/SKILL.md` in this repository, and the `atm` binary embeds them:
`atm agents install` writes the skills of the version that runs, where each harness reads them. People who haven't
installed atm yet, and harnesses atm doesn't install into, find skills through public repositories instead
(`npx skills add <owner>/<repo>`). So each release should publish atm's skills to one (T-453).

That needs three choices: which repository, how it is laid out, and how it is versioned. Other apps of ours may
publish skills later, so the choice shouldn't be atm's alone.

## Proposal

1. **One repository for the organization: `solo-swe/skills`**, shared by every app of ours that publishes skills.
2. **Layout:** `skills/<name>/SKILL.md` (with the skill's `references/`), the Agent Skills format. Each app's skills
   carry its prefix (`atm-…`), so names never clash and a user picks one app's with `--skill`. The README has a
   section per app saying what its skills need (for atm's: atm installed, and `atm agents install` as the better way
   to get them).
3. **Each app's release writes only its own part.** atm's release (`packaging/release.sh` and
   `.github/workflows/release.yml`, T-453) replaces the `skills/atm-*` folders with this version's `skills/` (the
   portable core: no Claude Code `allowed-tools`, no `/atm:` links), updates its README section, commits, and tags the
   commit `atm-v<version>`. It never touches another app's folders.
4. **`main` is what users get.** The `atm-v<version>` tags mark which atm a commit's skills came with; users don't
   install from them. Matching skills to the installed atm is `atm agents install`'s job.
5. **Access:** the release pushes with a deploy key of `solo-swe/skills` (a repository secret here), as the Homebrew
   formula's update does with `solo-swe/homebrew-tap`; nothing else writes to it. Until the repository and the key
   exist, the release skips this step with a message instead of failing.

## Alternatives considered

- **A repository for atm alone (`solo-swe/atm-skills`).** Plain version tags (`v0.33.0`), and installing it gives
  exactly atm's skills. Discarded: it isn't what the well-known publishers do (one repository per organization, see the
  appendix), a second app would need a second repository, release setup and deploy key, and users would have to find
  each app's repository separately. Its one advantage, plain tags, matters little while users install from `main`.
- **The skills in this repository, installed from a subfolder** (`npx skills add solo-swe/agentic-task-manager`).
  No second repository and no export step. Discarded: users would get our development copy on `main` (ahead of any
  release, and with the generated Claude Code variant beside it), installing from a subfolder isn't documented for
  `npx skills`, and the source repository would become part of the distribution's surface.
- **Users install a version, by tag.** Each release tagged, users pinning `atm-v0.33.0`. Discarded: installing from a
  tag isn't documented for `npx skills`, and none of the publishers we looked at version their skills repository by a
  product's version; `atm agents install` already gives skills that match the installed atm.
- **A Claude Code plugin marketplace** (as `anthropics/skills` also is). Discarded for now: atm leaves the plugin
  behind at the switch (porting-to-rust.md §6), and a marketplace serves one harness. The repository's layout doesn't
  rule it out later.

## Risks

- **Skills and atm out of step.** A skill installed with `npx skills` can be newer or older than the user's atm.
  Mitigation: each skill checks atm's version first (T-441) and says when they disagree; the README section points to
  `atm agents install`.
- **One app's release breaks another's skills.** Mitigation: a release touches only its own prefixed folders and its
  README section; the release script refuses to change anything else.
- **A second external repository to look after.** It needs its deploy key rotated and its access kept to the release.
  Mitigation: the same setup as the Homebrew tap, listed in `packaging/README.md`.
- **The `npx skills` CLI changes.** If it or the harnesses start installing from tags or subfolders, or per-app
  repositories become the convention, a new ADR replaces this one.

## Consequences

- `npx skills add solo-swe/skills` lists every app's skills; atm's docs say
  `npx skills add solo-swe/skills --skill atm-…` (or the interactive pick), and point to `atm agents install` first.
- A second app adds its own `<app>-*` folders and `<app>-v<version>` tags, with no change to atm's release.
- Hakan creates `solo-swe/skills` and its deploy key before T-453's release step can run.

## Appendix: how others publish skills

Checked 2026-11-08.

| Repository | Shape | Versions |
|---|---|---|
| [anthropics/skills](https://github.com/anthropics/skills) | One repo for the organization, `skills/<name>/SKILL.md`, many unrelated skills; also a Claude Code plugin marketplace grouping them into plugins | No tags or releases: `main` is the source |
| [vercel-labs/agent-skills](https://github.com/vercel-labs/agent-skills) | One repo, skills for several products (React, Next.js, Vercel deployment, writing) | A release for every change on `main`, not tied to a product's version |
| `supabase/agent-skills`, `expo/skills` | One repo per company, `skills/<name>/` | — |
| Cloudflare, Stripe | Also repos per product (`cloudflare/wrangler`, `stripe/stripe-best-practices`) | With the product |

The `npx skills` CLI ([vercel-labs/skills](https://github.com/vercel-labs/skills)) installs from a repository's
default branch, lets the user pick skills in a repository of many (`--skill <name>`, repeatable), and refreshes them
with `npx skills update`. Installing from a tag or a subfolder isn't documented.
