## docs/documents-design.md:26 · round 1 · t1
> - **At most 5,000 documents**; past that, the first 5,000 by path and a line saying how many more there are

- [x] **Hakan Serce** · 2026-11-23T23:47:31Z · c1
  500 is more than enough (5000 is way too much)
  - **claude:ba530ebb** · 2026-11-23T23:48:24Z · c3
    Done: the cap is now 500. Past that, the list shows the first 500 by path and says how many more there are. Search then asks the route (`?q=`), which matches before it cuts, so every document can still be found (§2, §4, §5, §6.5).

## docs/documents-design.md:24 · round 1 · t2
> - **Submodules:** not entered. A submodule is listed as a folder you can't open, named as one; its documents are

- [x] **Hakan Serce** · 2026-11-23T23:47:46Z · c2
  include submodule documents
  - **claude:ba530ebb** · 2026-11-23T23:48:25Z · c4
    Done: submodule documents are included. A checked-out submodule is a folder marked as a submodule, with its documents found by the same rules in its own repository (tracked files, plus untracked ones its rules don’t ignore), just as the viewer already reads a file inside one (T-408). Nested submodules are included too. One that isn’t checked out shows as an empty folder that says so (§2, §5, §6.4).
