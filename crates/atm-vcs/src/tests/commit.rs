//! Committing: the paths named, the changes under a folder, the commit.

use super::*;

#[test]
fn whole_tree_pathspecs_are_refused() {
    let vcs = GitVcs::new(&std::env::temp_dir());
    for name in WHOLE_TREE.iter().copied().chain([":/x", ":(glob)*.rs"]) {
        let error = vcs.paths(&[name.to_string()], None).unwrap_err();
        assert!(
            matches!(&error, Error::Invalid(m) if m.ends_with("would stage the whole tree")),
            "{name}: {error:?}"
        );
    }
}

#[test]
fn the_files_to_commit_are_named_relative_to_here_once_each() {
    let dir = scratch();
    let vcs = init(&dir.0);
    write(&vcs, "src/a.rs", "a");
    write(&vcs, "b.txt", "b");
    let picked = vcs
        .paths(
            &names(&["a.rs", "../b.txt", "./a.rs", "../src/a.rs"]),
            Some(&dir.0.join("src")),
        )
        .unwrap();
    assert_eq!(picked, ["src/a.rs", "b.txt"]);
    assert_eq!(vcs.paths(&[], None).unwrap(), Vec::<String>::new());
}

#[test]
fn a_name_outside_the_repository_a_folder_or_a_file_git_doesnt_know_is_refused() {
    let dir = scratch();
    let vcs = init(&dir.0.join("repo"));
    write(&vcs, "src/a.rs", "a");
    std::fs::write(dir.0.join("outside.txt"), "x").unwrap();
    // (the name, the message)
    for (name, message) in [
        ("../outside.txt", "../outside.txt is outside the repository"),
        ("..", ".. is outside the repository"),
        ("src/..", "src/.. is outside the repository"),
        ("src", "src is a directory: name the files in it"),
        (
            "missing.txt",
            "missing.txt: no such file, and git doesn't track it",
        ),
    ] {
        let error = vcs.paths(&names(&[name]), None).unwrap_err();
        assert_eq!(error.to_string(), message, "{name}");
    }
}

#[test]
fn a_deleted_file_git_knows_can_be_named() {
    let dir = scratch();
    let vcs = init(&dir.0);
    write(&vcs, "by-hand.txt", "a");
    write(&vcs, "git-rm.txt", "b");
    commit_all(&vcs, "first");
    std::fs::remove_file(dir.0.join("by-hand.txt")).unwrap();
    git(&dir.0, &["rm", "-q", "git-rm.txt"]);
    assert_eq!(
        vcs.paths(&names(&["by-hand.txt", "git-rm.txt"]), None)
            .unwrap(),
        ["by-hand.txt", "git-rm.txt"]
    );
}

#[test]
fn a_name_is_a_literal_path_never_a_pattern() {
    let dir = scratch();
    let vcs = init(&dir.0);
    write(&vcs, "a.env", "secret");
    write(&vcs, "b.env", "secret");
    commit_all(&vcs, "first");
    write(&vcs, "a.env", "changed");
    write(&vcs, "b.env", "changed");
    // `*.env` names no file: refused, not every .env file
    let error = vcs.paths(&names(&["*.env"]), None).unwrap_err();
    assert_eq!(
        error.to_string(),
        "*.env: no such file, and git doesn't track it"
    );
    vcs.commit(&names(&["a.env"]), "only a").unwrap();
    let status = git(&dir.0, &["status", "--porcelain"]);
    assert_eq!(status, "M b.env");
}

#[test]
fn a_commit_holds_exactly_the_files_named_and_what_else_is_staged_stays_staged() {
    let dir = scratch();
    let vcs = init(&dir.0);
    write(&vcs, "a.txt", "a");
    write(&vcs, "gone.txt", "g");
    let first = commit_all(&vcs, "first");
    write(&vcs, "a.txt", "a2");
    write(&vcs, "new.txt", "n");
    write(&vcs, "staged.txt", "s");
    git(&dir.0, &["add", "staged.txt"]);
    git(&dir.0, &["rm", "-q", "gone.txt"]);
    let sha = vcs
        .commit(&names(&["a.txt", "new.txt", "gone.txt"]), "the change")
        .unwrap();
    assert_eq!(vcs.head().as_deref(), Some(sha.as_str()));
    assert_eq!(vcs.resolve(&Rev::head()), Some(sha.clone()));
    assert_eq!(vcs.message(&r(&sha)).unwrap(), "the change");
    let changed = git(&dir.0, &["show", "--name-status", "--format=", &sha]);
    assert_eq!(changed, "M\ta.txt\nD\tgone.txt\nA\tnew.txt");
    assert_eq!(git(&dir.0, &["status", "--porcelain"]), "A  staged.txt");
    assert!(vcs.in_history(&r(&first)) && vcs.in_history(&r(&sha)));
}

#[test]
fn a_commit_git_refuses_says_what_git_said() {
    let dir = scratch();
    let vcs = init(&dir.0);
    write(&vcs, "a.txt", "a");
    commit_all(&vcs, "first");
    let error = vcs.commit(&names(&["a.txt"]), "nothing").unwrap_err();
    assert!(
        error.to_string().starts_with("git commit failed: "),
        "{error}"
    );
    assert!(
        vcs.message(&r("no-such-commit"))
            .unwrap_err()
            .to_string()
            .starts_with("git log failed: ")
    );
}

#[test]
fn changes_under_a_folder_are_what_git_status_lists_there_renames_with_both_names() {
    let dir = scratch();
    let vcs = init(&dir.0);
    write(
        &vcs,
        "docs/old.md",
        "same text, long enough to be seen as a rename\n",
    );
    write(&vcs, "docs/kept.md", "k");
    write(&vcs, "docs-other/x.md", "x");
    write(&vcs, ".gitignore", "*.log\n");
    commit_all(&vcs, "first");
    git(&dir.0, &["mv", "docs/old.md", "docs/new.md"]);
    write(&vcs, "docs/kept.md", "k2");
    write(&vcs, "docs/deep/added.md", "a");
    write(&vcs, "docs/build.log", "ignored");
    write(&vcs, "docs-other/x.md", "x2");
    let mut found = vcs.changes_under(&names(&["docs"]), None).unwrap();
    found.sort();
    assert_eq!(
        found,
        [
            "docs/deep/added.md",
            "docs/kept.md",
            "docs/new.md",
            "docs/old.md"
        ]
    );
    // From a folder inside, and two folders, each change once
    let from_inside = vcs
        .changes_under(
            &names(&["deep", "../docs-other", "deep"]),
            Some(&dir.0.join("docs")),
        )
        .unwrap();
    assert_eq!(from_inside, ["docs/deep/added.md", "docs-other/x.md"]);
}

#[test]
fn changes_under_refuse_the_root_above_it_the_task_folder_a_file_and_a_clean_folder() {
    let dir = scratch();
    let vcs = init(&dir.0.join("repo"));
    write(&vcs, "clean/a.txt", "a");
    write(&vcs, &format!("{}/tasks.json", atm_common::TASK_DIR), "{}");
    commit_all(&vcs, "first");
    write(
        &vcs,
        &format!("{}/tasks.json", atm_common::TASK_DIR),
        "{\"tasks\": []}",
    );
    // (the folder, the message's start)
    for (name, message) in [
        (".", "--dir .: name a folder inside the repository"),
        ("..", "--dir ..: name a folder inside the repository"),
        (
            atm_common::TASK_DIR,
            "--dir .agentic-task-manager: the task folder isn't committed this way",
        ),
        (
            "clean/a.txt",
            "--dir clean/a.txt is a file: name it with --file",
        ),
        ("clean", "--dir clean: nothing changed under it"),
        ("missing", "--dir missing: nothing changed under it"),
    ] {
        let error = vcs.changes_under(&names(&[name]), None).unwrap_err();
        assert!(error.to_string().starts_with(message), "{name}: {error}");
    }
}
