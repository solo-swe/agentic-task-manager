+++
id = "01a1eca9-4bfc-7663-8d69-71405e87a6d4"
number = 3
title = "Hidden files show in a review s diffs"
status = "approved"
tasks = ["ATM-598"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T03:18:00Z
updated = 2026-11-18T06:44:23Z
+++

Round 2: the lines you pointed at (c2, c4) are gone. There is one rule now, `document_path`, for every read: a review's sides at a revision and the document viewer's working copy alike.

- **Shown:** hidden files and folders (`.github/workflows/ci.yml`, `.gitignore`, `.prettierrc`, `.env.example`).
- **Still refused:** git's and atm's own folders (`.git`, `.agentic-task-manager`), environment files (`.env`, `.env.local`, …), key files (`*.pem`, `id_rsa*`, …), and, in the working copy, anything git ignores.
- `revision_path` is removed; `read_document_at` uses `document_path`.
- **Security doc:** I11 now states the one rule for `documents` and `files`. `docs/multi-project-api.md` lists the refusals.

Tests:
- atm-docs: the shown and refused table on the one rule. A hidden file is read at a revision and from the working copy, and a hidden file git ignores stays refused. The image case now refuses `.git/x.png`.
- `serve.test.mjs`: `.gitignore` opens through `documents`; `.env`, `.git/config` and `.agentic-task-manager/tasks.json` stay 403.
- The e2e refusal message follows the new wording.
