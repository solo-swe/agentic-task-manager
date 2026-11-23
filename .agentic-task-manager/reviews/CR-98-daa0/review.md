+++
id = "01a1fb14-4481-737e-bb15-ba2e0921daa0"
number = 98
title = "Dashboard links and storage: submodule link, raw HTML, area and SVG links, . and .., blocked storage, malformed %"
status = "approved"
tasks = ["ATM-676"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T22:29:31Z
updated = 2026-11-20T22:40:39Z
+++

The audit's D3, D4, D5, D7 and D9.

- **D3:** a review's submodule commit link goes through `submoduleCommitUrl` (`repo.ts`). That's safeUrl, the host's own commit path (GitHub, GitLab, Bitbucket, Gitea), and no trailing `.git`. An SSH address or an unknown host gets no link.
- **D4:** I checked CompMD's own policy first, in its published README. Raw HTML is `details`, `summary`, `sub`, `sup`, `kbd`, `br`, `img` and `a` with safe URLs, and anything else shows as text.
  - safeTree now applies that list itself, less `img`, as defence in depth. A kept tag carrying `on…`, `style`, `src` and the like is text too.
  - `secureLinks`, CompMd's click handler, the app's external-link catcher and the new-tab clicks share one `LINK_SELECTOR`, which also covers `area[href]` and SVG links (`href` or `xlink:href`).
- **D5:** `sessionStore()` returns null when reading `sessionStorage` throws, so the app-tabs module loads with site data blocked.
- **D7:**
  - `addressParams` gives no page for a path with a `.` or `..` segment, `%2E%2E` included.
  - `idSegment()` refuses those IDs in every API path builder.
  - A review change is refused before it's sent if its workspace, project, review or comment ID is one of them.
- **D9:** `safeDecode` returns null for a malformed `%`, so `#task=%E0` and a bad anchor no longer throw.

Security doc: I14 extended.

To check: the raw-HTML rule in `compmd.ts` (`rawHtmlKept`). Inline, each tag is its own node, so only opening tags are judged.

Not in this change: a full unit run once hit an unhandled rejection from CompMD's settling after the HelpDialog test file ended. It's older than this change and recorded as ATM-709.
