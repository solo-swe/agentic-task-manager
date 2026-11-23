## (the review) · round 1 · t1
- [x] **Hakan Serce** · 2026-11-18T05:45:09Z · c1
  I am not sure if this is related with moving a submodule. I believe it is actually related with just referring to a file change under a submodule. Can you write a test to check if we can actually show a file diff from a submodule as part of a parent repo changeset shows contents in a CR?
  - **claude:0f56a916** · 2026-11-18T05:50:04Z · c2
    You were right: that was the bigger case. The test you asked for (`a_project_commit_moving_a_submodule_shows_the_files_changed_inside_it`, in `crates/atm-lib/src/reviews.rs`) failed: a project commit moving a submodule to a later commit listed only the submodule (`lib`), none of the files changed inside it. Round 2 fixes it. Under the submodule s entry, the page now lists each file its repository changed between the two commits, named as the project has them (`lib/a.txt`), each with its diff and its sides read at the submodule s commits; the file tree shows them under the submodule. The test passes, reading both sides. Round 1 s case (a submodule only moved, its commit the same) stays fixed. Comments on those files anchor as on any file; across later rounds they show where they were made (not followed through the submodule yet).

## crates/atm-lib/src/review_pages.rs:293 · round 2 · t2
>             if file.submodule.is_none() && !file.patch.contains("\n@@") {

- [x] **Hakan Serce** · 2026-11-18T05:52:01Z · c3
  what is the meaning of the second condition here? when do we have that?
  - **claude:0f56a916** · 2026-11-18T05:52:32Z · c4
    It means the entry has no lines (no @@ hunk). Only such an entry can be a submodule git did not mark: one whose commit changed has its Subproject commit lines and is found by the first condition already. We have no lines for a pure rename (the moved submodule, CompMD s markdown-rs), a mode change, an empty file or a binary file, and only those are looked up in the trees, one git process each; files with lines never are, so a large review costs nothing more. Round 3 names it (no_lines) and says this in a comment there.
