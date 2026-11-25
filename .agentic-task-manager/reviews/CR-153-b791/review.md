+++
id = "01a20df7-b5cf-73a6-acae-1780af4bb791"
number = 153
title = "App: ipcFetch hands the parsed answer through"
status = "approved"
tasks = ["ATM-769"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T14:31:07Z
updated = 2026-11-24T16:38:16Z
+++

In the app, a command's answer reaches the page once, as `invoke` parsed it (ATM-769, from the memory analysis §3b, CR-140). It used to be turned back into text and parsed again: about three copies of every answer, two in WebKit's own memory.

**The change:**
- `api/ipc.ts`: a JSON answer is now `answered(body, status)`, a `Response`-shaped object whose `json()` returns `body` itself. `ok`, `status` and `headers` work as before. `text()` and `clone()` are there in case something calls them, and only build text when asked. Images still come as a real `Response` (they need `blob()`).
- `api/http.ts`: `fetchFile` now returns the parsed JSON (`data`) instead of the text (`text`), so the client never reads an answer as text. `missing` is now a value (`{ tasks: [] }`) instead of a JSON string. In the browser, `resp.json()` parses atm serve's answer once, the same as `JSON.parse(text)` did.
- `load.ts`, `worktrees.ts`: read `file.data`. The tasks size fallback (`text.length`) is gone because the tasks route always sends `bytes` (`archive.rs`).

**One test changed (`app.spec.ts`, right-click menus):** a card's copied link now includes `sprint=active`. Answers arrive a few ticks sooner, so the page writes its Sprint filter into the address (T-323, which it does for any project with sprints) after the opening link lands, not before. Before, the link replaced the address after that write, so the address lost `?sprint=active` while the filter stayed on. The test now waits for the address to show the filter and expects it in the link. That earlier ordering (a link opened in the app dropping the Sprint filter from the address) looks like a small existing quirk. Want a task for it?

**Tests:**
- `ipc.test.ts`: the answer comes through as the same object (twice, through `fetchFile` too, and from a clone). `text()` only on request. `ok` is true only for a 2xx status. A command's error still comes through the client as its status, or as `missing`.
- `http.test.ts`: a 200 is its parsed JSON with its validators. A 200 that isn't JSON (empty too) rejects. The parsed object is handed through. `missing` is a value.
- `pnpm run check`, the unit suite (2005 passed), the functional e2e suite (612 passed, the webkit and timed projects included).
