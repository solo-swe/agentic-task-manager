//! The remote: the base branch, the origin's address, a file as last fetched.

use super::*;

#[test]
fn the_remote_gives_the_default_base_its_address_and_files_as_last_fetched() {
    let dir = scratch();
    let vcs = init(&dir.0);
    write(&vcs, "a.txt", "on the remote\n");
    commit_all(&vcs, "first");
    assert_eq!(
        vcs.file_on_remote_branch("main", "a.txt"),
        None,
        "no remote yet"
    );
    git(
        &dir.0,
        &["remote", "add", "origin", "https://github.com/o/r.git"],
    );
    git(&dir.0, &["update-ref", "refs/remotes/origin/trunk", "HEAD"]);
    git(
        &dir.0,
        &[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/trunk",
        ],
    );
    assert_eq!(
        vcs.remote_url().as_deref(),
        Some("https://github.com/o/r.git")
    );
    assert_eq!(vcs.default_base(), "trunk");
    assert_eq!(
        vcs.file_on_remote_branch("trunk", "a.txt").as_deref(),
        Some("on the remote\n")
    );
    // A branch atm doesn't accept, a path read as an option, a file or a branch the remote hasn't: none
    for (branch, path) in [
        ("trunk", "missing.txt"),
        ("other", "a.txt"),
        ("a..b", "a.txt"),
        ("-x", "a.txt"),
        ("trunk", "-p"),
    ] {
        assert_eq!(
            vcs.file_on_remote_branch(branch, path),
            None,
            "{branch} {path}"
        );
    }
}
