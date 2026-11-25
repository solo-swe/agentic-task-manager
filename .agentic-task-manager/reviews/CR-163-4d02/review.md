+++
id = "01a20e5d-39fc-76ee-ab85-50de21844d02"
number = 163
title = "Documents: the listing in atm-docs"
status = "approved"
tasks = ["ATM-764"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T16:22:00Z
updated = 2026-11-24T16:37:56Z
+++

The documents listing (ATM-764, [the documents design §2–§3](docs/documents-design.md#2-which-files)): which files, their titles, submodules, the cap and the query. It lives in atm-docs, with git's two listings in atm-vcs and the call in atm-lib. The route, the app's command and the page are their own tasks.

**atm-vcs (`files.rs`):**
- `listed_files()`: what the repository tracks (`ls-files -z --stage`) and what it has untracked and doesn't ignore by its own rules (`ls-files -z --others --exclude-standard`). Both use literal pathspecs and NUL separators, so no name is quoted or split.
  - The submodules it records (mode 160000) come back too, each saying whether it is checked out (its folder has a `.git`).
  - A conflicted file is listed once.
  - An untracked folder that is a repository of its own (`dir/`) is neither listed nor entered.
  - None outside a repository.
- `read_file_start(path, limit)`: a file's first bytes, through the same no-symlink check as `read_file`.

**atm-docs (`listing.rs`):**
- `list_documents(vcs, q)` gives `{git, documents: [{path, title, bytes, modified_at, tracked}], submodules: [{path, checked_out}], more}`:
  - **Which files:** `*.md` and `*.markdown`, and only those `document_path` accepts (not `.git/`, `.agentic-task-manager/`, environment or key files). Each must be a regular file reached through no symlink, as the viewer reads it.
  - **Submodules:** each checked-out one is listed from its own repository by its own rules, nested ones too (at most 8 deep), with paths from the project's root. One not checked out is listed with `checked_out: false` and no documents.
  - **Titles** (`document_title`): the first level-one heading in the first 8 KB, ATX (`# Title`, a closing `#` run dropped) or Setext (`===`), past front matter and outside fenced code. Else the file's name without `.md`/`.markdown`. Capped at 200 characters.
  - **At most 500** by path, `more` the rest. `q` (at most 200 characters, every word in the title or the path, any case) is matched before the cut. Only the listed documents are read for titles without a query; with one, all of them are.
  - **Without git:** `{git: false, documents: [], …}`. The `git` field is my addition to §5's shape, so the page can say the project needs git. The route task can drop or document it.
- `documents_stamp(vcs)`: each repository's `HEAD`, and every document's path, size, time and tracking, the submodules' too, for the route's `ETag`.

**atm-lib:** `Member::documents(q)` and `Member::documents_stamp()`, beside `Member::document`.

**Tests:**
- `atm-docs/src/listing/tests.rs`:
  - titles of each kind (ATX, spaced, closed, `C#`, Setext with trailing spaces, level two and `#tag` not titles, front matter, fences of both kinds, indented code, empty headings, `.markdown`, capped);
  - tracked, untracked, ignored and non-Markdown files; the viewer's refusals and a symlink;
  - submodules: checked out, nested, untracked and ignored inside one by its own rules, not checked out;
  - the cap with `more`, and `q` past the cut by title (any case) and path, every word, cut too;
  - a query of 201 characters refused;
  - no git;
  - the stamp: unchanged, not moved by a non-document, moved by a new document, by staging it and by a commit.
- `atm-vcs/src/tests/files.rs`: tracked and untracked (spaces and Unicode in names, one ignored); the submodule recorded and checked out; a nested repository not entered; the submodule's own files; not checked out after `deinit`; `read_file_start`; outside a repository.
- `atm-lib` (`workspaces.rs`): a member's documents and its stamp.
- clippy, fmt, `cargo test` for atm-vcs, atm-docs, atm-lib and atm-api.
