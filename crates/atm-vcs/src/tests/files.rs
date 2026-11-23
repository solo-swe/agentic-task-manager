//! Files: the working copy's and a commit's, ignore rules and tracking, the working state, submodules.

use super::*;

#[test]
fn a_working_copy_file_is_read_only_below_the_root_through_no_link() {
    let dir = scratch();
    let vcs = init(&dir.0.join("repo"));
    write(&vcs, "docs/a.md", "hello");
    assert_eq!(vcs.read_file("docs/a.md").unwrap(), b"hello");
    let (size, mtime) = vcs.file_stamp("docs/a.md").unwrap();
    assert_eq!(size, 5);
    assert!(mtime > 0);
    for missing in ["docs/none.md", "docs", ""] {
        assert_eq!(vcs.file_stamp(missing), None, "{missing:?}");
        assert_eq!(
            vcs.read_file(missing).unwrap_err(),
            Error::NotFound(format!("{missing}: no such file"))
        );
    }
    #[cfg(unix)]
    {
        std::fs::write(dir.0.join("outside.md"), "secret").unwrap();
        std::os::unix::fs::symlink(dir.0.join("outside.md"), vcs.root.join("link.md")).unwrap();
        std::os::unix::fs::symlink(vcs.root.join("docs"), vcs.root.join("linked")).unwrap();
        for linked in ["link.md", "linked/a.md"] {
            assert_eq!(vcs.file_stamp(linked), None, "{linked}");
            assert!(
                matches!(vcs.read_file(linked), Err(Error::NotFound(_))),
                "{linked}"
            );
        }
    }
}

#[test]
fn a_commits_regular_file_has_its_size_and_blob_and_anything_else_none() {
    let dir = scratch();
    let vcs = init(&dir.0);
    write(&vcs, "docs/a.md", "hello");
    write(&vcs, "run.sh", "#!/bin/sh\n");
    git(&dir.0, &["add", "-A"]);
    git(&dir.0, &["update-index", "--chmod=+x", "run.sh"]);
    #[cfg(unix)]
    std::os::unix::fs::symlink("docs/a.md", vcs.root.join("link.md")).unwrap();
    let head = r(&commit_all(&vcs, "first"));
    let (size, blob) = vcs.blob_in(&head, "docs/a.md").unwrap();
    assert_eq!(size, 5);
    assert_eq!(vcs.blob_bytes(&r(&blob)).unwrap(), b"hello");
    assert_eq!(
        vcs.blob_in(&head, "run.sh").map(|(s, _)| s),
        Some(10),
        "executable"
    );
    for other in ["docs", "missing.md", "docs/a", "link.md"] {
        assert_eq!(vcs.blob_in(&head, other), None, "{other}");
    }
    assert_eq!(vcs.blob_in(&r("no-such-commit"), "docs/a.md"), None);
    let unknown = "0".repeat(40);
    assert_eq!(
        vcs.blob_bytes(&r(&unknown)).unwrap_err(),
        Error::NotFound(format!("{unknown}: no such blob"))
    );
}

#[test]
fn a_tracked_file_is_never_ignored_and_ignore_rules_decide_the_rest() {
    let dir = scratch();
    let vcs = init(&dir.0);
    write(&vcs, ".gitignore", "*.log\n");
    write(&vcs, "kept.log", "tracked before the rule mattered");
    git(&dir.0, &["add", "-f", "kept.log", ".gitignore"]);
    git(&dir.0, &["commit", "-q", "-m", "first"]);
    write(&vcs, "new.log", "n");
    write(&vcs, "new.txt", "n");
    // (the file, ignored, tracked)
    for (file, ignored, tracked) in [
        ("kept.log", false, true),
        ("new.log", true, false),
        ("new.txt", false, false),
        ("missing.txt", false, false),
    ] {
        assert_eq!(vcs.ignored(file), ignored, "{file}");
        assert_eq!(vcs.tracked(file), Some(tracked), "{file}");
    }
}

/// ATM-607: a burst of changes counts when any file in it is one git doesn't ignore, in one git run
/// ATM-607: the working copy's state changes with a file of the work written, and only then
#[test]
fn the_working_state_changes_with_the_work_only() {
    let dir = scratch();
    let vcs = init(&dir.0);
    write(&vcs, ".gitignore", "*.log\n");
    write(&vcs, "a.txt", "one\n");
    git(&dir.0, &["add", "."]);
    git(&dir.0, &["commit", "-q", "-m", "first"]);
    let clean = vcs.working_state().unwrap();
    assert_eq!(vcs.working_state().unwrap(), clean, "nothing changed");
    write(&vcs, "build.log", "ignored");
    write(&vcs, ".agentic-task-manager/tasks.json", "{}");
    assert_eq!(
        vcs.working_state().unwrap(),
        clean,
        "an ignored file, or atm's folder"
    );
    write(&vcs, "a.txt", "two\n");
    let edited = vcs.working_state().unwrap();
    assert_ne!(edited, clean, "a file of the work edited");
    std::thread::sleep(std::time::Duration::from_millis(20));
    write(&vcs, "a.txt", "three\n");
    assert_ne!(vcs.working_state().unwrap(), edited, "edited again");
    write(&vcs, "new.txt", "n");
    assert!(
        vcs.working_state().unwrap().contains("new.txt"),
        "a new file"
    );
    assert_eq!(GitVcs::new(&dir.0.join("not-a-repo")).working_state(), None);
}

/// ATM-670 (audit H1): a rename's old path, the record after it, lost three bytes (`README.md` read as `DME.md`),
/// and one starting with a multibyte character panicked
#[test]
fn a_staged_rename_is_read_with_both_its_paths_whole() {
    let dir = scratch();
    let vcs = init(&dir.0);
    write(&vcs, "éé/README.md", "# Read me\n");
    git(&dir.0, &["add", "."]);
    git(&dir.0, &["commit", "-q", "-m", "first"]);
    git(&dir.0, &["mv", "éé/README.md", "éé/x.md"]);
    let state = vcs.working_state().unwrap();
    // The new path after its status, the old one whole after it
    assert!(state.contains("R  éé/x.md\0éé/README.md\t"), "{state:?}");
    // The old path moved too: a different state
    git(&dir.0, &["mv", "éé/x.md", "éé/y.md"]);
    assert_ne!(vcs.working_state().unwrap(), state);
}

#[test]
fn a_burst_counts_when_any_of_its_files_isnt_ignored() {
    let dir = scratch();
    let vcs = init(&dir.0);
    write(&vcs, ".gitignore", "*.log\ntarget/\n");
    write(&vcs, "kept.log", "tracked");
    git(&dir.0, &["add", "-f", "kept.log", ".gitignore"]);
    git(&dir.0, &["commit", "-q", "-m", "first"]);
    let burst = |paths: &[&str]| {
        vcs.any_not_ignored(&paths.iter().map(|p| (*p).to_string()).collect::<Vec<_>>())
    };
    assert!(!burst(&[]), "nothing changed");
    assert!(
        !burst(&["a.log", "target/debug/atm", "target/x"]),
        "all ignored"
    );
    assert!(burst(&["a.log", "src/main.rs"]), "one that isn't");
    assert!(
        burst(&["kept.log"]),
        "a tracked file, though its name is ignored"
    );
    assert!(burst(&["new.txt"]), "a new file, not ignored");
    assert!(
        GitVcs::new(&dir.0.join("not-a-repo")).any_not_ignored(&["a.log".into()]),
        "outside a repository every file counts"
    );
}

/// ATM-599: a submodule's link in a commit or a tree gives its commit; a file, a folder or nothing there, none
#[test]
fn a_submodules_link_in_a_commit_or_tree_names_its_commit() {
    let dir = scratch();
    let sub = init(&dir.0.join("sub-origin"));
    write(&sub, "a.txt", "a");
    let sub_head = commit_all(&sub, "sub");
    let vcs = init(&dir.0.join("repo"));
    write(&vcs, "lib/top.txt", "t");
    commit_all(&vcs, "first");
    git(
        &vcs.root,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "--quiet",
            "add",
            "../sub-origin",
            "lib/sub",
        ],
    );
    let head = commit_all(&vcs, "the submodule");
    let tree = vcs.resolve_tree(&r(&head)).unwrap();
    for object in [r(&head), r(&tree)] {
        assert_eq!(
            vcs.gitlink_in(&object, "lib/sub"),
            Some(sub_head.clone()),
            "{object}"
        );
        for other in ["lib/top.txt", "lib", "lib/none", "lib/sub/a.txt", "-p"] {
            assert_eq!(vcs.gitlink_in(&object, other), None, "{object} {other}");
        }
    }
}

#[test]
fn a_file_in_a_submodule_is_answered_by_the_submodules_own_repository() {
    let dir = scratch();
    let sub = init(&dir.0.join("sub-origin"));
    write(&sub, "a.txt", "a");
    write(&sub, ".gitignore", "*.tmp\n");
    let sub_head = commit_all(&sub, "sub");
    let vcs = init(&dir.0.join("repo"));
    write(&vcs, "top.txt", "t");
    commit_all(&vcs, "first");
    git(
        &vcs.root,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "--quiet",
            "add",
            "../sub-origin",
            "lib/sub",
        ],
    );
    commit_all(&vcs, "the submodule");
    write(&vcs, "lib/sub/new.tmp", "n");
    assert_eq!(
        vcs.tracked("lib/sub/a.txt"),
        Some(true),
        "tracked by the submodule"
    );
    assert_eq!(vcs.tracked("lib/sub/new.tmp"), Some(false));
    assert!(vcs.ignored("lib/sub/new.tmp"), "the submodule's rules");
    assert!(!vcs.ignored("lib/sub/a.txt"));
    let of = vcs.submodule_of("lib/sub/a.txt").unwrap();
    assert_eq!(of["path"], serde_json::json!("lib/sub"));
    assert_eq!(of["commit"], serde_json::json!(sub_head));
    assert_eq!(vcs.submodule_of("top.txt"), None);
    // A folder with a `.git` that this repository doesn't record as a submodule is no submodule
    let nested = init(&dir.0.join("repo/nested"));
    write(&nested, "b.txt", "b");
    assert_eq!(vcs.submodule_of("nested/b.txt"), None);
}
