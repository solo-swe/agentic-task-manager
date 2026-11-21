//! Reading a file: at a revision (a commit, a tree, a submodule's), from the working copy, with each check on the way.

use crate::*;

/// A throwaway repository: `a.md` committed twice (`one`, then `two`), the first commit tagged `first`.
struct Repo(std::path::PathBuf);

impl Repo {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("atm-docs-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let repo = Self(dir);
        repo.git(&["init", "-q"]);
        repo.git(&["config", "user.email", "t@example.invalid"]);
        repo.git(&["config", "user.name", "t"]);
        repo.commit("a.md", "# One\n");
        repo.git(&["tag", "first"]);
        repo.commit("a.md", "# Two\n");
        repo
    }

    fn git(&self, args: &[&str]) {
        assert!(atm_fs::git::git(args, &self.0).is_some(), "git {args:?}");
    }

    fn commit(&self, file: &str, text: &str) {
        std::fs::write(self.0.join(file), text).unwrap();
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", file]);
    }

    fn vcs(&self) -> GitVcs {
        GitVcs::new(&self.0)
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// ATM-583: a review's round of commits made in a submodule names them by the submodule's commits (or trees)
#[test]
fn a_file_in_a_submodule_at_one_of_its_revisions_is_read_from_its_own_repository() {
    let sub = Repo::new();
    let project = Repo::new();
    let source = sub.0.to_string_lossy().into_owned();
    project.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        "-q",
        &source,
        "lib",
    ]);
    project.git(&["commit", "-q", "-m", "add lib"]);
    let lib = Repo(project.0.join("lib"));
    lib.git(&["config", "user.email", "t@example.invalid"]);
    lib.git(&["config", "user.name", "t"]);
    lib.commit("a.md", "# Three, in lib\n");
    let head = atm_fs::git::git(&["rev-parse", "HEAD"], &lib.0).unwrap();
    let tree = atm_fs::git::git(&["rev-parse", "HEAD^{tree}"], &lib.0).unwrap();
    let vcs = project.vcs();
    let at_commit = read_document_at(Some(&vcs), &head, "lib/a.md").unwrap();
    assert_eq!(at_commit["text"], json!("# Three, in lib\n"));
    assert_eq!(at_commit["path"], json!("lib/a.md"));
    assert_eq!(at_commit["commit"], json!(head));
    let at_tree = read_document_at(Some(&vcs), &tree, "lib/a.md").unwrap();
    assert_eq!(at_tree["text"], json!("# Three, in lib\n"));
    // The project's own revisions still read the project's files; the submodule's name none of them
    assert_eq!(
        read_document_at(Some(&vcs), "first", "a.md").unwrap()["text"],
        json!("# One\n")
    );
    assert!(matches!(
        read_document_at(Some(&vcs), &head, "a.md"),
        Err(Error::NotFound(_))
    ));
    assert!(matches!(
        read_document_at(Some(&vcs), &head, "lib/none.md"),
        Err(Error::NotFound(_))
    ));
    std::mem::forget(lib); // inside the project, which removes it
}

/// The test repository's first commit, tagged `first`.
fn first() -> atm_vcs::Rev {
    atm_vcs::Rev::new("first").unwrap()
}

#[test]
fn a_file_at_a_revision_is_read_as_that_commit_has_it() {
    let repo = Repo::new();
    let vcs = repo.vcs();
    let then = read_document_at(Some(&vcs), "first", "a.md").unwrap();
    assert_eq!(then["document"], "# One\n");
    assert_eq!(then["kind"], "markdown");
    assert_eq!(then["revision"], json!(vcs.resolve(&first())));
    assert_eq!(
        read_document(Some(&vcs), "a.md").unwrap()["document"],
        "# Two\n"
    );
    // Code is fenced as it is from the working copy
    repo.commit("src.rs", "fn main() {}\n");
    let code = read_document_at(Some(&vcs), "HEAD", "src.rs").unwrap();
    assert_eq!(
        (code["language"].as_str(), code["line_offset"].as_u64()),
        (Some("rust"), Some(1))
    );
}

/// ATM-689: a review's image file shows its sides, each read as its revision has it
#[test]
fn an_image_at_a_revision_is_its_bytes_as_that_commit_has_them_only_when_it_is_one() {
    let repo = Repo::new();
    let vcs = repo.vcs();
    repo.commit("pic.gif", "GIF89a-old");
    repo.git(&["tag", "old"]);
    repo.commit("pic.gif", "GIF89a-new");
    assert_eq!(
        read_image_at(Some(&vcs), "old", "pic.gif").unwrap(),
        (b"GIF89a-old".to_vec(), "image/gif")
    );
    assert_eq!(
        read_image_at(Some(&vcs), "HEAD", "pic.gif").unwrap().0,
        b"GIF89a-new".to_vec()
    );
    // In a tree too (a round rebuilt from its diff)
    let tree = vcs
        .resolve_tree(&atm_vcs::Rev::new("old").unwrap())
        .unwrap();
    assert_eq!(
        read_image_at(Some(&vcs), &tree, "pic.gif").unwrap().0,
        b"GIF89a-old".to_vec()
    );
    // Not an image, or not the image its name says: refused; not there then: not found
    repo.commit("fake.png", "<html>");
    for (path, what) in [
        ("a.md", "not an image"),
        ("fake.png", "not the image its name says"),
    ] {
        assert!(
            matches!(
                read_image_at(Some(&vcs), "HEAD", path),
                Err(Error::Refused(_))
            ),
            "{what}"
        );
    }
    assert!(matches!(
        read_image_at(Some(&vcs), "first", "pic.gif"),
        Err(Error::NotFound(_))
    ));
    // The path's and the revision's checks, as every read at a revision
    assert!(matches!(
        read_image_at(Some(&vcs), "HEAD", "../pic.gif"),
        Err(Error::Invalid(_))
    ));
    assert!(matches!(
        read_image_at(Some(&vcs), "--output=x", "pic.gif"),
        Err(Error::Invalid(_))
    ));
}

/// A review's round rebuilt from its diff is a tree no commit has: read the same way, with no commit named
#[test]
fn a_file_in_a_tree_is_read_as_the_tree_has_it() {
    let repo = Repo::new();
    let vcs = repo.vcs();
    let tree = vcs.resolve_tree(&first()).unwrap();
    let then = read_document_at(Some(&vcs), &tree, "a.md").unwrap();
    assert_eq!(then["document"], "# One\n");
    assert_eq!(then["revision"], json!(tree));
    assert_eq!(then["commit"], Value::Null);
    // A commit's own answer names the commit
    let at_commit = read_document_at(Some(&vcs), "first", "a.md").unwrap();
    assert_eq!(at_commit["commit"], json!(vcs.resolve(&first())));
    // The tree's checks are a commit's: a file it doesn't have, a path atm doesn't show
    assert!(matches!(
        read_document_at(Some(&vcs), &tree, "b.md"),
        Err(Error::NotFound(_))
    ));
    assert!(matches!(
        read_document_at(Some(&vcs), &tree, ".env"),
        Err(Error::Refused(_))
    ));
}

/// ATM-598, CR-3: a hidden file is read at a revision (a review's side) and from the working copy (the viewer)
#[test]
fn a_hidden_file_is_read_at_a_revision_and_from_the_working_copy() {
    let repo = Repo::new();
    let vcs = repo.vcs();
    std::fs::create_dir_all(repo.0.join(".github/workflows")).unwrap();
    repo.commit(".github/workflows/ci.yml", "on: push\n");
    let side = read_document_at(Some(&vcs), "HEAD", ".github/workflows/ci.yml").unwrap();
    assert_eq!(side["text"], "on: push\n");
    let now = read_document(Some(&vcs), ".github/workflows/ci.yml").unwrap();
    assert_eq!(now["text"], "on: push\n");
    // A hidden file git ignores stays refused in the working copy
    repo.commit(".gitignore", ".cache/\n");
    std::fs::create_dir_all(repo.0.join(".cache")).unwrap();
    std::fs::write(repo.0.join(".cache/x.md"), "x\n").unwrap();
    assert!(matches!(
        read_document(Some(&vcs), ".cache/x.md"),
        Err(Error::Refused(_))
    ));
    // An environment file committed by mistake stays refused at a revision too
    repo.commit(".env", "TOKEN=x\n");
    assert!(matches!(
        read_document_at(Some(&vcs), "HEAD", ".env"),
        Err(Error::Refused(_))
    ));
}

#[test]
fn a_file_at_a_revision_passes_the_same_checks() {
    let repo = Repo::new();
    let vcs = repo.vcs();
    // The path's checks first, whatever the commit
    assert!(matches!(
        read_document_at(Some(&vcs), "HEAD", ".env"),
        Err(Error::Refused(_))
    ));
    assert!(matches!(
        read_document_at(Some(&vcs), "HEAD", "../a.md"),
        Err(Error::Invalid(_))
    ));
    // A revision is never an option for git
    for bad in ["-p", "--output=x", ""] {
        assert!(
            matches!(
                read_document_at(Some(&vcs), bad, "a.md"),
                Err(Error::Invalid(_))
            ),
            "{bad}"
        );
    }
    assert!(matches!(
        read_document_at(Some(&vcs), "no-such-ref", "a.md"),
        Err(Error::NotFound(_))
    ));
    // Not in that commit; a folder isn't a file
    assert!(matches!(
        read_document_at(Some(&vcs), "first", "b.md"),
        Err(Error::NotFound(_))
    ));
    std::fs::create_dir_all(repo.0.join("docs")).unwrap();
    repo.commit("docs/x.md", "x\n");
    assert!(matches!(
        read_document_at(Some(&vcs), "HEAD", "docs"),
        Err(Error::NotFound(_))
    ));
    // A file the ignore rules name: as the working copy answers (git ignores no file it tracks), whether the commit
    // has it or not
    std::fs::write(repo.0.join(".gitignore"), "secret.txt\nlocal.md\n").unwrap();
    std::fs::write(repo.0.join("secret.txt"), "s\n").unwrap();
    repo.git(&["add", "-f", "secret.txt", ".gitignore"]);
    repo.git(&["commit", "-q", "-m", "secret"]);
    assert_eq!(
        read_document_at(Some(&vcs), "HEAD", "secret.txt").is_ok(),
        read_document(Some(&vcs), "secret.txt").is_ok()
    );
    // Ignored and untracked now, committed in an older commit (before the rule): refused, as from the working copy
    std::fs::write(repo.0.join("local.md"), "l\n").unwrap();
    repo.git(&["add", "-f", "local.md"]);
    repo.git(&["commit", "-q", "-m", "local"]);
    repo.git(&["rm", "-q", "--cached", "local.md"]);
    repo.git(&["commit", "-q", "-m", "untrack"]);
    assert!(matches!(
        read_document_at(Some(&vcs), "HEAD~1", "local.md"),
        Err(Error::Refused(_))
    ));
    assert!(matches!(
        read_document(Some(&vcs), "local.md"),
        Err(Error::Refused(_))
    ));
}

#[test]
fn without_a_working_copy_nothing_is_read_but_a_bad_path_is_still_named() {
    assert!(matches!(
        read_document(None, "a.md"),
        Err(Error::NotFound(_))
    ));
    assert!(matches!(
        document_stamp(None, "a.md"),
        Err(Error::NotFound(_))
    ));
    assert!(matches!(read_image(None, "a.png"), Err(Error::NotFound(_))));
    assert!(matches!(
        read_document_at(None, "HEAD", "a.md"),
        Err(Error::NotFound(_))
    ));
    // The path's checks come first
    assert!(matches!(
        read_document(None, ".env"),
        Err(Error::Refused(_))
    ));
    assert!(matches!(
        read_document_at(None, "HEAD", "../a"),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn a_working_copy_file_is_read_with_its_size_time_tracking_and_commit() {
    let repo = Repo::new();
    let vcs = repo.vcs();
    let tracked = read_document(Some(&vcs), "a.md").unwrap();
    assert_eq!(tracked["bytes"], 6);
    assert_eq!(tracked["tracked"], true);
    assert_eq!(tracked["commit"], json!(vcs.head()));
    assert_eq!(tracked["submodule"], Value::Null);
    assert!(
        tracked["modified_at"]
            .as_str()
            .is_some_and(|t| t.starts_with("20")),
        "{}",
        tracked["modified_at"]
    );
    std::fs::write(repo.0.join("new.txt"), "n\n").unwrap();
    let untracked = read_document(Some(&vcs), "new.txt").unwrap();
    assert_eq!(untracked["tracked"], false);
    assert_eq!(untracked["kind"], "text");
    // The stamp is the file's size, from the same checks
    assert_eq!(document_stamp(Some(&vcs), "new.txt").unwrap().0, 2);
    assert!(matches!(
        document_stamp(Some(&vcs), ".env"),
        Err(Error::Refused(_))
    ));
}

#[test]
fn a_working_copy_path_that_isnt_a_file_there_is_not_found() {
    let repo = Repo::new();
    let vcs = repo.vcs();
    std::fs::create_dir_all(repo.0.join("docs")).unwrap();
    for missing in ["b.md", "docs", "docs/none.md"] {
        assert!(
            matches!(read_document(Some(&vcs), missing), Err(Error::NotFound(_))),
            "{missing}"
        );
    }
}

#[test]
fn a_file_git_ignores_is_refused_from_the_working_copy() {
    let repo = Repo::new();
    let vcs = repo.vcs();
    std::fs::write(repo.0.join(".gitignore"), "build/\n").unwrap();
    std::fs::create_dir_all(repo.0.join("build")).unwrap();
    std::fs::write(repo.0.join("build/out.txt"), "o\n").unwrap();
    assert!(matches!(
        read_document(Some(&vcs), "build/out.txt"),
        Err(Error::Refused(_))
    ));
    assert!(matches!(
        document_stamp(Some(&vcs), "build/out.txt"),
        Err(Error::Refused(_))
    ));
}

#[test]
fn a_document_up_to_1_mb_and_an_image_up_to_5_mb_are_read_and_larger_ones_refused() {
    let repo = Repo::new();
    let vcs = repo.vcs();
    let limit = usize::try_from(DOC_MAX_BYTES).unwrap();
    let image_limit = usize::try_from(DOC_MAX_IMAGE_BYTES).unwrap();
    std::fs::write(repo.0.join("at.txt"), vec![b'a'; limit]).unwrap();
    std::fs::write(repo.0.join("past.txt"), vec![b'a'; limit + 1]).unwrap();
    assert!(read_document(Some(&vcs), "at.txt").is_ok());
    assert!(matches!(
        read_document(Some(&vcs), "past.txt"),
        Err(Error::TooLarge(_))
    ));
    assert!(matches!(
        document_stamp(Some(&vcs), "past.txt"),
        Err(Error::TooLarge(_))
    ));
    // An image has a limit of its own
    let png = |len: usize| {
        let mut data = b"\x89PNG\r\n\x1a\n".to_vec();
        data.resize(len, 0);
        data
    };
    std::fs::write(repo.0.join("big.png"), png(limit + 1)).unwrap();
    std::fs::write(repo.0.join("at.png"), png(image_limit)).unwrap();
    std::fs::write(repo.0.join("past.png"), png(image_limit + 1)).unwrap();
    assert_eq!(
        read_document(Some(&vcs), "big.png").unwrap()["kind"],
        "image"
    );
    assert!(read_image(Some(&vcs), "at.png").is_ok());
    assert!(matches!(
        read_image(Some(&vcs), "past.png"),
        Err(Error::TooLarge(_))
    ));
    // As at a revision
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-q", "-m", "large"]);
    assert!(matches!(
        read_document_at(Some(&vcs), "HEAD", "past.txt"),
        Err(Error::TooLarge(_))
    ));
    assert!(matches!(
        read_document_at(Some(&vcs), "HEAD", "past.png"),
        Err(Error::TooLarge(_))
    ));
    assert!(read_document_at(Some(&vcs), "HEAD", "big.png").is_ok());
}

#[test]
fn an_image_file_is_served_only_when_its_bytes_are_the_image_its_name_says() {
    let repo = Repo::new();
    let vcs = repo.vcs();
    std::fs::write(repo.0.join("a.gif"), b"GIF89a....").unwrap();
    std::fs::write(repo.0.join("fake.png"), b"<script>").unwrap();
    assert_eq!(
        read_image(Some(&vcs), "a.gif").unwrap(),
        (b"GIF89a....".to_vec(), "image/gif")
    );
    assert!(matches!(
        read_image(Some(&vcs), "fake.png"),
        Err(Error::Refused(_))
    ));
    // Anything but an image, even one the viewer shows, isn't served as a file
    assert!(matches!(
        read_image(Some(&vcs), "a.md"),
        Err(Error::Refused(_))
    ));
    assert!(matches!(
        read_image(Some(&vcs), "none.png"),
        Err(Error::NotFound(_))
    ));
    // Inside git's own folder, refused before anything is read (a hidden image elsewhere is shown, CR-3)
    assert!(matches!(
        read_image(Some(&vcs), ".git/x.png"),
        Err(Error::Refused(_))
    ));
}

#[test]
fn outside_git_a_file_is_read_with_no_tracking_and_no_commit() {
    let dir = std::env::temp_dir().join(format!("atm-docs-plain-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.md"), "# A\n").unwrap();
    let vcs = GitVcs::new(&dir);
    let read = read_document(Some(&vcs), "a.md");
    let _ = std::fs::remove_dir_all(&dir);
    let read = read.unwrap();
    assert_eq!(read["document"], "# A\n");
    assert_eq!(
        (&read["tracked"], &read["commit"]),
        (&Value::Null, &Value::Null)
    );
}

#[test]
fn a_revision_with_a_nul_is_invalid_and_a_binary_file_in_a_commit_says_so() {
    let repo = Repo::new();
    let vcs = repo.vcs();
    assert!(matches!(
        read_document_at(Some(&vcs), "HEAD\0", "a.md"),
        Err(Error::Invalid(_))
    ));
    std::fs::write(repo.0.join("data.txt"), b"a\0b").unwrap();
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-q", "-m", "binary"]);
    let binary = read_document_at(Some(&vcs), "HEAD", "data.txt").unwrap();
    assert_eq!(binary["kind"], "binary");
    assert_eq!(binary["text"], Value::Null);
    assert_eq!(binary["bytes"], 3);
    assert_eq!(binary["tracked"], true);
    assert_eq!(binary["modified_at"], Value::Null);
}

#[cfg(unix)]
#[test]
fn a_symlink_in_the_working_copy_is_not_a_file_nor_a_folder_on_the_way_to_one() {
    let repo = Repo::new();
    let vcs = repo.vcs();
    std::os::unix::fs::symlink(repo.0.join("a.md"), repo.0.join("link.md")).unwrap();
    std::fs::create_dir_all(repo.0.join("real")).unwrap();
    std::fs::write(repo.0.join("real/b.md"), "b\n").unwrap();
    std::os::unix::fs::symlink(repo.0.join("real"), repo.0.join("via")).unwrap();
    for linked in ["link.md", "via/b.md"] {
        assert!(
            matches!(read_document(Some(&vcs), linked), Err(Error::NotFound(_))),
            "{linked}"
        );
    }
    assert!(read_document(Some(&vcs), "real/b.md").is_ok());
}

#[cfg(unix)]
#[test]
fn a_symlink_in_a_commit_is_not_a_file() {
    let repo = Repo::new();
    std::os::unix::fs::symlink("/etc/passwd", repo.0.join("link.md")).unwrap();
    repo.git(&["add", "link.md"]);
    repo.git(&["commit", "-q", "-m", "link"]);
    assert!(matches!(
        read_document_at(Some(&repo.vcs()), "HEAD", "link.md"),
        Err(Error::NotFound(_))
    ));
}
