+++
id = "01a1ecb3-5906-70c6-83f9-bbfc387f1a31"
number = 4
title = "A submodule only moved shows as the submodule, at its commit"
status = "approved"
tasks = ["ATM-599"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T03:28:59Z
updated = 2026-11-18T05:58:46Z
+++

A review hid a submodule's change (reported by Hakan, 2026-11-18, on CompMD's CR-4): the file tree listed `crates/markdown-rs`, but its diff said it wasn't a file of the commit.

## Cause
CR-4 moves the `markdown-rs` submodule into `crates/`, its commit unchanged. git writes that as a rename with no `Subproject commit` lines, so the page took it for an ordinary file and asked for its sides, which are a submodule's link, not a file.

## Fix
- `atm-vcs`: `gitlink_in(object, path)`, the commit a submodule is at in a commit or a tree.
- `review_pages.rs`: an entry whose patch has no hunks is checked against its sides' trees; a submodule's link there makes it the submodule's entry, at its commits.
- `ReviewFile.svelte`: a submodule whose commit stays the same says that its folder moved, and where from.

## Tests
- atm-vcs: a link in a commit and in a tree names its commit; a file, a folder or nothing there, none.
- atm-lib: a commit that only moves a submodule shows it as the submodule at its one commit.
- The card: *its folder moved*, said with its commit.
