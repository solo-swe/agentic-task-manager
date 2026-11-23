+++
id = "01a1f082-322d-7048-81f4-86386bbe2d7d"
number = 25
title = "A later round shows what changed, not files deleted or wholly added"
status = "approved"
tasks = ["ATM-622"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T21:13:46Z
updated = 2026-11-18T21:19:53Z
+++

What you saw: round 2's whole change showed the changed file entirely as added, and *since round 1* showed every other file as deleted.

**The cause:** a files round, made by `atm review round CR-n --file X` where X was only the file that changed.
- A files round reviews its files whole, with no base. Its whole change is each file shown as added (that part is by design: there is no "before" for a file reviewed whole).
- Its tree is rebuilt from the files it names alone. So the comparison with round 1, over the files either round had, read every file round 2 didn't name as deleted.

**The fix, in two places:**
1. **`atm review round --file X` keeps the files the last files round reviewed** (naming more adds them; a file deleted since is dropped). A round is again the whole review as it now is, and *since round 1* is just what changed.
2. **`rounds::compare`:** when the later round is a files round, only the files it names are compared. A file of the earlier round it doesn't name is left out, not shown as deleted. This also repairs reviews already made this way, like the one you were reading.

**For range rounds** (commits): comparing two range rounds was already right, since it diffs the two heads' trees. But `atm docs reviews` now tells agents that a round is the whole change as it now is: a range round keeps the review's first base (`--range <first base>..HEAD`), never the fix commit's own range. My CR-13 round 2 broke that rule: its whole change was the fix alone.

Which review were you looking at? If it wasn't a files round, tell me and I'll look at that one.

**Tests:**
- atm-review: a files round naming fewer files compares only what it has. It failed before the fix, showing `notes.md` deleted.
- The CLI suite: a later `--file` round keeps the earlier files, *since round 1* is only the changed line, and a file deleted since isn't kept.
- The whole review suite (23 tests), clippy and fmt pass.
- No e2e test: the dashboard isn't changed, and its fixture plays atm's answers, so it can't test this.
