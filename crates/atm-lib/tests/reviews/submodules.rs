//! Reviews of commits that move a submodule (ATM-599).

use serde_json::Value;

use crate::fixtures::{dir, git, project, reopen};
use atm_lib::reviews::{NewReview, ReviewRef, Status};

/// ATM-599, Hakan on CR-4: a commit of the project that moves a submodule to a later commit shows the files that
/// changed inside it, each with its diff and its two sides, as the submodule's commits have them
#[test]
fn a_project_commit_moving_a_submodule_shows_the_files_changed_inside_it() {
    let dir = dir();
    let sub = dir.0.join("sub-origin");
    std::fs::create_dir_all(&sub).unwrap();
    git(&["init", "-q"], &sub);
    std::fs::write(sub.join("a.txt"), "a\n").unwrap();
    git(&["add", "-A"], &sub);
    git(&["commit", "-q", "-m", "sub"], &sub);
    let root = dir.0.join("repo");
    std::fs::create_dir_all(&root).unwrap();
    git(&["init", "-q"], &root);
    let project = project(&root, "claude:1");
    let add = [
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "--quiet",
        "add",
        "../sub-origin",
        "lib",
    ];
    git(&add, &root);
    git(&["add", "-A"], &root);
    git(&["commit", "-q", "-m", "base"], &root);
    // In the submodule: a file changed, one added, committed there; the project records its new commit
    std::fs::write(root.join("lib/a.txt"), "a, changed\n").unwrap();
    std::fs::write(root.join("lib/b.txt"), "b\n").unwrap();
    git(&["add", "-A"], &root.join("lib"));
    git(&["commit", "-q", "-m", "inside"], &root.join("lib"));
    git(&["add", "lib"], &root);
    git(&["commit", "-q", "-m", "the submodule moved on"], &root);
    let vcs = project.vcs().unwrap();
    let first =
        atm_lib::reviews::rounds::from_range(&vcs, "HEAD~1", "HEAD", &"claude:1".into()).unwrap();
    let mut agent = reopen(&root, "claude:1");
    let header = atm_lib::reviews::create(
        &mut agent,
        NewReview {
            title: "Moved on".into(),
            summary: String::new(),
            tasks: vec!["T-1".into()],
            author: None,
            reviewers: Vec::new(),
            status: Status::Waiting,
            first,
        },
    )
    .unwrap();
    let page =
        atm_lib::review_pages::review_page(&agent, &ReviewRef(header.reference.0), None, None)
            .unwrap();
    let files = page["view"]["files"].as_array().unwrap();
    let paths: Vec<&str> = files.iter().filter_map(|f| f["path"].as_str()).collect();
    // The submodule's entry, and the files inside it
    assert!(paths.contains(&"lib"), "{paths:?}");
    let inside = |path: &str| {
        files
            .iter()
            .find(|f| f["path"] == path)
            .unwrap_or_else(|| panic!("{path} not shown: {paths:?}"))
    };
    let a = inside("lib/a.txt");
    assert_eq!(a["status"], "modified");
    let patch = a["patch"].as_str().unwrap();
    assert!(
        patch.contains("-a\n") && patch.contains("+a, changed\n"),
        "{patch}"
    );
    assert_eq!(inside("lib/b.txt")["status"], "added");
    // Each side reads as the submodule's commits have it
    let read = |revision: &Value| {
        atm_lib::documents::read_document_at(Some(&vcs), revision.as_str().unwrap(), "lib/a.txt")
            .unwrap()["text"]
            .clone()
    };
    assert_eq!(read(&a["before"]), "a\n");
    assert_eq!(read(&a["after"]), "a, changed\n");
}

/// ATM-599 (`markdown-rs` moved into `crates/`, in CompMD): a commit that only moves a submodule's folder shows it as
/// the submodule, at its one commit, not as a file whose sides are read
#[test]
fn a_submodule_only_moved_shows_as_the_submodule_at_its_commit() {
    let dir = dir();
    let sub = dir.0.join("sub-origin");
    std::fs::create_dir_all(&sub).unwrap();
    git(&["init", "-q"], &sub);
    std::fs::write(sub.join("a.txt"), "a\n").unwrap();
    git(&["add", "-A"], &sub);
    git(&["commit", "-q", "-m", "sub"], &sub);
    let root = dir.0.join("repo");
    std::fs::create_dir_all(&root).unwrap();
    git(&["init", "-q"], &root);
    let project = project(&root, "claude:1");
    let add = [
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "--quiet",
        "add",
        "../sub-origin",
        "lib",
    ];
    git(&add, &root);
    git(&["add", "-A"], &root);
    git(&["commit", "-q", "-m", "base"], &root);
    std::fs::create_dir_all(root.join("crates")).unwrap();
    git(&["mv", "lib", "crates/lib"], &root);
    git(&["commit", "-q", "-m", "moved"], &root);
    let vcs = project.vcs().unwrap();
    let first =
        atm_lib::reviews::rounds::from_range(&vcs, "HEAD~1", "HEAD", &"claude:1".into()).unwrap();
    let mut agent = reopen(&root, "claude:1");
    let header = atm_lib::reviews::create(
        &mut agent,
        NewReview {
            title: "Moved".into(),
            summary: String::new(),
            tasks: vec!["T-1".into()],
            author: None,
            reviewers: Vec::new(),
            status: Status::Waiting,
            first,
        },
    )
    .unwrap();
    let page =
        atm_lib::review_pages::review_page(&agent, &ReviewRef(header.reference.0), None, None)
            .unwrap();
    let files = page["view"]["files"].as_array().unwrap();
    let moved = files.iter().find(|f| f["path"] == "crates/lib").unwrap();
    assert_eq!(moved["old_path"], "lib");
    let (from, to) = (&moved["submodule"]["from"], &moved["submodule"]["to"]);
    assert!(from.is_string() && from == to, "{moved}");
    // The other entries are files, as before
    assert!(
        files
            .iter()
            .filter(|f| f["path"] != "crates/lib")
            .all(|f| f["submodule"].is_null())
    );
}
