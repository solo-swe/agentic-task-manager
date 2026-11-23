+++
id = "01a1fafc-518e-7777-ab2a-6502aed57bef"
number = 96
title = "Document routes: .git in any case, logo rules, other sites refused, full object IDs"
status = "approved"
tasks = ["ATM-671"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T22:03:22Z
updated = 2026-11-20T22:40:04Z
+++

The audit's document-route findings (H2, H5, H6, H7, H8):

- **H2:** the documents and files routes refuse git's and atm's own folders in any case (`.GIT`) and as Windows reads them (`.git.`, `.git `).
- **H5:** a GET or HEAD from another site's page is refused (403) when `Sec-Fetch-Site` is `cross-site` or the `Origin` isn't on this machine; the page itself and `/assets/` are still served. Reads write `recent.json` and run git, so another site shouldn't be able to trigger them.
- **H6:** `?at=` is a full object ID only (40 or 64 hex); a short ID, a moving name or an option is a 400, and atm-lib isn't asked. A full subtree ID is still accepted (git checks it).
- **H7:** the logo follows the files route's rules: no symbolic link on the way (even inside the project), never in `.git` or `.agentic-task-manager`.
- **H8:** a logo that can't be read is a 404, not a cached empty 200.

Security doc: I11 and I12 updated.

To check: the 403 rule in `crates/atm-api/src/lib.rs` (`from_another_site`), and whether refusing every non-object-ID `at` breaks a review page you use.
