## (the review) · round 1 · t1
- [x] **Hakan Serce** · 2026-11-18T06:08:07Z · c1
  Is there a way to just watch for changes, rather than asking every 2 seconds (maybe web can poll, app can watch?)
  - **claude:0f56a916** · 2026-11-18T06:08:49Z · c2
    Yes, and your split is the right one:

    - **App: watch.** It already gets pushes: the task store and the review store are watched (`atm_fs::watch`, the subscription behind T-506). A live review would add one more watcher, on the project folder itself, recursive, leaving out `.git/` and anything git ignores. A settled burst (300 ms) tells the page, and the page asks for `view.now` once. No polling, and a change shows up in about half a second.
    - **Browser: polling for now.** `atm serve` has no push channel. Adding one (server-sent events) is a new entry point, so it needs its own design and an update to the security doc. Meanwhile I would make each poll cheap: answer with an `ETag` built from the working tree state (`git status` porcelain plus the files changed mtimes), so an unchanged copy costs a `304` and no diff.

    I would make this a follow-up task (P2, this sprint) rather than grow CR-8: the watcher in the app, and the ETag on the route. Shall I add it?
    - **Hakan Serce** · 2026-11-18T06:09:39Z · c3
      create the task (in this sprint) and share task id here.
      - **claude:0f56a916** · 2026-11-18T06:11:01Z · c4
        Created: **ATM-607**, *Live reviews follow the working copy by watching it in the app, and by cheap polls in the browser* (P2, in this sprint). The app gets a watcher on the project folder that pushes to the page; the browser keeps polling, but with an ETag, so an unchanged copy costs a 304.
