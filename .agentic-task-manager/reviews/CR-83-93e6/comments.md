## packaging/README.md:31 · round 1 · t1
> etches main and tags its commit on GitHub (origin/main) v<version>, once that commit's Cargo.toml has the version and the tag exists nowhere yet (and pushes the tag with --push). It never tags HEAD, so it runs from any branch and working tree (ATM-705)

- [x] **Hakan Serce** · 2026-11-20T17:06:33Z · c1
  what happens if we are already working on main?
  - **claude:0f56a916** · 2026-11-20T17:07:14Z · c2
    It works the same there: it fetches and tags `origin/main`, never your local `main`. So: (1) **main up to date** (or behind): tags what is on GitHub, as before. (2) **main with commits not pushed**: they are not in the release. It tags the pushed commit and now says so (`Not in it: main's N commit(s) not pushed to origin yet.`); your commits stay as they are. (3) **the version bump itself not pushed**: it refuses, since `origin/main`'s `Cargo.toml` doesn't have the version yet; the message now says to merge the release PR *or push main* first. Tests for (2) and (3), and the README line here says it. In d4c5f8e.

## (the review) · round 2 · t2
- [x] **Hakan Serce** · 2026-11-20T17:07:47Z · c3
  are we missing something here? I am feeling we are reinventing the wheel. What is the industry standard for doing this? In the end it is just basic tagging, right?
  - **claude:0f56a916** · 2026-11-20T17:08:58Z · c4
    You're right: it is just `git tag`. And `release.yml` already does the one check that matters. Both of its build jobs refuse a tag whose `Cargo.toml` isn't at that version ("The tag is the workspace's version"). So the script duplicates CI, plus a few local guards.

    **What the industry does**, from simplest to most automated:
    1. **A plain tag**, with CI checking it: `git tag -a v0.43.0 origin/main -m 'atm 0.43.0'` then `git push origin v0.43.0`, or `gh release create`. Most projects that hand-tag do this, and the check lives in CI, as ours does.
    2. **Tag on merge**: a workflow on pushes to `main` tags the version when it has no tag yet. release-please and release-plz (Rust's usual one) work this way. They also write the version bump and changelog, which we write ourselves (CHANGELOG plus WHATS-NEW). The catch: a tag pushed with CI's own token doesn't start other workflows, so it needs a deploy key or token with write access. That is a CI permission change, so it is your call.
    3. **cargo-release**: bumps, tags and publishes crates. We publish no crates (`publish = false`), so most of it doesn't apply.

    **My recommendation: (1).**
    - Delete `packaging/release.sh` and its tests.
    - Write the two commands in `packaging/README.md` and `AGENTS.md`.
    - Add one guard to `release.yml`: the tagged commit is on `main` (`git merge-base --is-ancestor`). That is the only thing the script checked that CI doesn't.

    (2) removes the step entirely, but it is more machinery and a new secret for a once-a-sprint command. Say which you want and I'll do it in this task.
    - **Hakan Serce** · 2026-11-20T17:11:05Z · c5
      ok, let's go with 1 for now, but if it is going to become pain again, we will switch to 2
