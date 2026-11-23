//! The history: revisions, ancestry, the commit first adding a file.

use super::*;

#[test]
fn revisions_resolve_to_their_commit_or_tree_and_only_ancestors_are_in_history() {
    let dir = scratch();
    let vcs = init(&dir.0);
    write(&vcs, "a.txt", "a");
    let first = commit_all(&vcs, "first");
    let tree = git(&dir.0, &["rev-parse", "HEAD^{tree}"]);
    assert_eq!(vcs.resolve(&r("main")), Some(first.clone()));
    assert_eq!(vcs.resolve_tree(&r(&first)), Some(tree.clone()));
    assert_eq!(
        vcs.resolve_tree(&r(&tree)),
        Some(tree.clone()),
        "a tree is its own"
    );
    assert_eq!(vcs.resolve(&r(&tree)), None, "a tree is no commit");
    assert_eq!(vcs.resolve(&r("no-such-branch")), None);
    git(&dir.0, &["checkout", "-q", "-b", "side"]);
    write(&vcs, "b.txt", "b");
    let side = commit_all(&vcs, "side");
    git(&dir.0, &["checkout", "-q", "main"]);
    assert!(vcs.in_history(&r(&first)));
    assert!(!vcs.in_history(&r(&side)), "on another branch");
    assert!(!vcs.in_history(&r("no-such-commit")));
}

#[test]
fn the_commit_first_adding_a_file_is_the_oldest_in_heads_history() {
    let dir = scratch();
    let vcs = init(&dir.0);
    assert_eq!(vcs.first_commit_adding("a.md"), None, "no commit yet");
    write(&vcs, "a.md", "one");
    let added = commit_all(&vcs, "add");
    write(&vcs, "a.md", "two");
    commit_all(&vcs, "change");
    assert_eq!(vcs.first_commit_adding("a.md"), Some(added.clone()));
    // Removed and added again: still the first
    git(&dir.0, &["rm", "-q", "a.md"]);
    commit_all(&vcs, "remove");
    write(&vcs, "a.md", "three");
    commit_all(&vcs, "again");
    assert_eq!(vcs.first_commit_adding("a.md"), Some(added));
    for other in ["missing.md", "-a.md", "*.md"] {
        assert_eq!(vcs.first_commit_adding(other), None, "{other}");
    }
}
