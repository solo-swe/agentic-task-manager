//! atm-vcs' unit tests, by the concern of the code they test, on throwaway repositories (git is the unit's own
//! dependency): the shared fixtures, and the tests of the crate as a whole, are here.

use super::*;

mod commit;
mod files;
mod history;
mod paths;
mod remote;

/// A folder of its own, removed when the test ends.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn scratch() -> Scratch {
    // A count too: tests run at once, and two can start in the same nanosecond
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let count = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "atm-vcs-git-{}-{count}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    // Without Windows' `\\?\` prefix, as atm and git name the folder
    Scratch(atm_common::pathtext::canonical(&dir).unwrap())
}

/// Run git in `cwd`, which must work: its output.
fn git(cwd: &Path, args: &[&str]) -> String {
    step(args, cwd, false).unwrap()
}

/// A repository at `dir`, with a committer of its own (commit runs git as users do, without `-c`).
fn init(dir: &Path) -> GitVcs {
    std::fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "-q", "-b", "main"]);
    for (key, value) in [
        ("user.name", "t"),
        ("user.email", "t@example.com"),
        ("commit.gpgsign", "false"),
        ("protocol.file.allow", "always"),
    ] {
        git(dir, &["config", key, value]);
    }
    GitVcs::new(dir)
}

fn write(vcs: &GitVcs, relative: &str, text: &str) {
    let path = vcs.root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn commit_all(vcs: &GitVcs, message: &str) -> String {
    git(&vcs.root, &["add", "-A"]);
    git(&vcs.root, &["commit", "-q", "-m", message]);
    git(&vcs.root, &["rev-parse", "HEAD"])
}

fn names(names: &[&str]) -> Vec<String> {
    names.iter().map(ToString::to_string).collect()
}

/// A revision, by a name or ID the test has.
fn r(name: &str) -> Rev {
    Rev::new(name).unwrap()
}

#[test]
fn only_git_is_a_version_control_atm_knows() {
    assert!(open(Path::new("."), "git").is_ok());
    let error = open(Path::new("."), "hg").unwrap_err();
    assert_eq!(
        error,
        Error::Invalid("the version control \"hg\" isn't one atm knows (git)".into())
    );
}

#[test]
fn outside_a_repository_git_answers_nothing() {
    let dir = scratch();
    std::fs::write(dir.0.join("a.txt"), "a").unwrap();
    let vcs = GitVcs::new(&dir.0);
    assert!(!vcs.in_repository());
    assert_eq!(vcs.head(), None);
    assert_eq!(vcs.tracked("a.txt"), None);
    assert!(!vcs.ignored("a.txt"));
    assert_eq!(vcs.submodule_of("a.txt"), None);
    assert_eq!(vcs.remote_url(), None);
    assert_eq!(vcs.default_base(), "main");
    assert_eq!(vcs.resolve(&Rev::head()), None);
}
