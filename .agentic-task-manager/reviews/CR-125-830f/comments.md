## (the review) · round 1 · t1
- [x] **Hakan Serce** · 2026-11-23T09:12:19Z · c1
  Why don't we just watch the folders for changes, and eliminate all polling? At the same time we would get instant updates instead of 5s or 1m, right?
  - **claude:0f56a916** · 2026-11-23T09:13:39Z · c2
    Yes. The reviews folder is already watched, so a comment or a decision is heard at once; the 5 s and 1 min looks only covered what that watch can't see. I'll make those watched too and drop the timed looks:

    - **The task folder** (one folder, not its subfolders): a task the agent takes or finishes changes which comments are its work.
    - **The working copy, only while a live review is open:** an edited file is what makes a comment on a live review the agent's. It uses the filter live reviews already use (ATM-607), so `.git/`, `target/` and other ignored files don't wake it. Without a live review, nothing there matters to the watch.
    - **A missed event:** macOS and Linux say so when their queue overflows (a rescan). I'll count that as a change rather than poll for it.

    Then a watch looks only when something it reads changed. It sleeps until its timeout otherwise, and also checks its shell. `atm review wait` gets the same change. I'll add it as a round here.
    - **Hakan Serce** · 2026-11-23T09:20:21Z · c3
      why do I not see the round 2 of the review in the diffs?
      - **claude:0f56a916** · 2026-11-23T09:20:36Z · c4
        Round 2 isn’t there yet: the change is written and its tests pass, but I haven’t committed it or added the round. The workspace tests and the CLI suite are running first; once they’re green I’ll commit and add round 2 (a few minutes). Until then the diffs show round 1 only.
      - **claude:0f56a916** · 2026-11-23T09:31:01Z · c5
        Round 2 is up now (fef3122): the timed looks are gone. The task file and, while a live review is open, the working copy are watched; missed events count as a change. The workspace tests and the CLI suite pass.
