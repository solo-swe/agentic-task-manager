## (the review) · round 1 · t1
- [x] **claude:0f56a916** · 2026-11-18T05:46:21Z · c1
  You sent this back with no comment, so I do not know what to change. Is it the rule (which hidden files a review shows, and that .env stays refused), the files it covers, or something you saw in the app? A word here and I will take it from there.

## crates/atm-docs/src/lib.rs:132-136 · round 1 · t2
>     let path = path_shape(path)?;
>     if path.split('/').any(|s| s.starts_with('.')) {
>         return Err(Error::Refused(format!(
>             "{path}: hidden files and folders aren't shown"
>         )));

- [x] **Hakan Serce** · 2026-11-18T05:49:13Z · c2
  Isn't this literally the problem?
  - **claude:0f56a916** · 2026-11-18T05:50:15Z · c3
    Yes, those lines are the cause. Before this round a review s sides went through them too, which is why every hidden file failed (and still does in your app, on 0.40.0). This round reads a side at a revision through `revision_path` instead, which shows hidden files and refuses only `.git`, `.agentic-task-manager`, `.env` files and key files. The lines you point at are now only the document viewer s rule for the working copy, which still refuses every hidden file. Should the viewer take the same narrower rule, so `.github/workflows/ci.yml` or `.gitignore` open there too? Files git ignores would stay refused, as now. It changes security invariant I11, so I would update the security document with it. Say yes and I will do it in this review.
    - **Hakan Serce** · 2026-11-18T06:39:30Z · c4
      then why do we still have thse lines here?
      - **claude:0f56a916** · 2026-11-18T06:42:00Z · c5
        They shouldn’t be: gone in round 2. There is one rule now, for the viewer and a review’s sides alike: hidden files are shown; `.git`, `.agentic-task-manager`, environment files and key files are refused, and in the working copy so is anything git ignores. Security invariant I11 updated with it.
