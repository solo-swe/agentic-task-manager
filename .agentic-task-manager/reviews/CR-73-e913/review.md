+++
id = "01a1f85b-beb5-74b2-a2a5-f69d531ee913"
number = 73
title = "A review shows its image files: before and after"
status = "approved"
tasks = ["ATM-689"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T09:48:44Z
updated = 2026-11-20T09:50:59Z
+++

A review's image files show their images. A binary file is still not copied into a round's diff, but an image of a type the viewer knows (PNG, JPEG, GIF, WebP, SVG) shows its sides on the Diffs tab:
- the after image where it was added;
- the before image where it was removed;
- both side by side where it changed.

Before, all of these said *A binary file: listed, not shown.* Other binary files still do. CR-60's screenshots are an example.

## Change
- **`atm-docs`:** `read_image_at(vcs, revision, path)`, an image's bytes as a commit or tree has it.
  - It makes `read_document_at`'s checks: the path's, a regular file (no symlink, no submodule), not one git ignores, under 5 MB, and the revision never an option for git.
  - Then `read_image`'s: only the image types, and the bytes must match the name.
  - The two reads share one lookup (`file_at`), so their checks can't drift apart.
- **atm-lib, atm-api:** `Member::image_at`, and `…/files?path=&at=<revision>`.
  - **Headers:** the same sandboxing CSP and `nosniff` as `files?path=`.
  - **Caching:** at a full object ID it is tagged and kept for a year, as `documents?…&at=` is; at any other revision it is neither tagged nor kept.
- **The app:** the `file` command takes `at` too, through the same atm-lib call (I27).
- **The dashboard:**
  - `v1FileUrl(project, path, at)`, and the app's call for it.
  - A new `ReviewImage.svelte` draws one side, labelled Before or After, on a checkerboard so transparent edges show. It loads through `imageSource`, so in the app the image comes through the command as a `data:` URL (the page's CSP allows only that).
  - `ReviewFile` shows the sides of a binary file whose name is an image type.
- **Docs:** `security-architecture.md` I11 (the files route with `&at=`, its checks) and `multi-project-api.md` §2.6.

## Tests
- **atm-docs:** an image read at a tag, at HEAD and in a tree. Refused when it isn't an image, or isn't the image its name says; not found at a revision before it existed. A path with `..` and a revision that is an option are invalid.
- **atm-api:** `files?…&at=`.
  - At a full object ID: sandboxed, kept, and a 304 when asked again without reading anything.
  - At `HEAD`: read each time, no-cache.
  - Each refusal answers with its status.
- **The app:** the `file` command at a revision answers what the route does (REST against command, and the main window's call).
- **E2e (`reviews.spec.ts`):**
  - an added PNG shows its After side and a changed one Before and After, each loaded (natural width 1) from its revision;
  - a `.wasm` file stays listed, not shown.
- **Checks:**
  - **Rust:** clippy on the changed crates; the tests of atm-docs (30), atm-api (63) and the app (111).
  - **Dashboard:** `pnpm run check`, the unit tests (1,819), and the reviews, documents and app specs (143).
