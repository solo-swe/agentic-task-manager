//! Running git, for the questions the store asks of a working copy (where its top level is): the Python atm's
//! `host/git.py` `git()`. Only atm-db and atm-vcs run git (docs/porting-to-rust.md §2).

use std::path::Path;
use std::process::{Command, Stdio};

use atm_common::py::strip;

/// Run git in `cwd`: its output, stripped, or None when git fails or isn't installed (a question, not a step).
#[must_use]
pub fn git(args: &[&str], cwd: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| strip(&String::from_utf8_lossy(&output.stdout)).to_string())
}

/// How long a working copy's place in git is remembered ([`where_in_git`]): long enough for one command, which asks
/// several times, short enough that the server (one long process) sees a folder become a repository soon after.
const REMEMBER: std::time::Duration = std::time::Duration::from_secs(5);

/// What git says of `cwd`'s place in it, remembered for a few seconds: `git rev-parse` with one query
/// (`--show-toplevel`, `--git-common-dir`), None outside git. One command asks the same several times, and each git
/// process is slow to start where processes are (Windows: a status change ran it seven times, ATM-482).
#[must_use]
pub fn where_in_git(query: &'static str, cwd: &Path) -> Option<String> {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    use std::time::Instant;
    type Answers = HashMap<(&'static str, std::path::PathBuf), (Instant, Option<String>)>;
    static ANSWERS: OnceLock<Mutex<Answers>> = OnceLock::new();
    let answers = ANSWERS.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (query, cwd.to_path_buf());
    if let Ok(remembered) = answers.lock()
        && let Some((asked, answer)) = remembered.get(&key)
        && asked.elapsed() < REMEMBER
    {
        return answer.clone();
    }
    let answer = git(&["rev-parse", query], cwd);
    if let Ok(mut answers) = answers.lock() {
        answers.insert(key, (Instant::now(), answer.clone()));
    }
    answer
}

/// The branch checked out per a git folder's `HEAD`; a detached `HEAD` gives its short SHA, no file None.
#[must_use]
pub fn head_branch(git_dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = strip(text.split('\n').next().unwrap_or_default());
    if let Some(reference) = head.strip_prefix("ref: ") {
        return Some(
            reference
                .strip_prefix("refs/heads/")
                .unwrap_or(reference)
                .to_string(),
        );
    }
    let short: String = head.chars().take(7).collect();
    (!short.is_empty()).then_some(short)
}

/// What "the branch" means everywhere in atm (a claim's, a project card's): the branch the working copy at
/// `worktree` has checked out, read from its `HEAD` without running git; a detached one gives its short SHA; None
/// outside git. A linked worktree's `.git` is a file naming its git folder.
#[must_use]
pub fn checked_out_branch(worktree: &Path) -> Option<String> {
    let dot_git = worktree.join(".git");
    if dot_git.is_file() {
        let text = std::fs::read_to_string(&dot_git).ok()?;
        let line = strip(text.split('\n').next().unwrap_or_default());
        return head_branch(&worktree.join(line.strip_prefix("gitdir: ")?));
    }
    head_branch(&dot_git)
}

#[cfg(test)]
mod tests {
    use super::{checked_out_branch, git};

    #[test]
    fn the_branch_is_read_from_head() {
        let dir = crate::test_dir::TestDir::new();
        let dot_git = dir.path().join(".git");
        std::fs::create_dir(&dot_git).unwrap();
        std::fs::write(dot_git.join("HEAD"), "ref: refs/heads/dev/1.0\n").unwrap();
        assert_eq!(checked_out_branch(dir.path()).as_deref(), Some("dev/1.0"));
        std::fs::write(dot_git.join("HEAD"), "0123456789abcdef\n").unwrap();
        assert_eq!(checked_out_branch(dir.path()).as_deref(), Some("0123456"));
        assert_eq!(checked_out_branch(&dir.path().join("none")), None);
    }

    #[test]
    fn a_head_outside_refs_heads_is_its_whole_reference_and_an_empty_one_is_none() {
        let dir = crate::test_dir::TestDir::new();
        for (head, branch) in [
            (
                "ref: refs/remotes/origin/main\n",
                Some("refs/remotes/origin/main"),
            ),
            ("  ref: refs/heads/main  \r\n", Some("main")),
            ("\n", None),
            ("", None),
        ] {
            std::fs::write(dir.path().join("HEAD"), head).unwrap();
            assert_eq!(
                super::head_branch(dir.path()).as_deref(),
                branch,
                "{head:?}"
            );
        }
    }

    #[test]
    fn a_linked_worktrees_branch_is_read_from_the_git_folder_its_dot_git_file_names() {
        let dir = crate::test_dir::TestDir::new();
        let admin = dir.path().join("repo/.git/worktrees/wt");
        std::fs::create_dir_all(&admin).unwrap();
        std::fs::write(admin.join("HEAD"), "ref: refs/heads/feature\n").unwrap();
        let worktree = dir.path().join("wt");
        std::fs::create_dir(&worktree).unwrap();
        // Relative to the worktree, as `git worktree add --relative-paths` writes it, or absolute
        for line in [
            "gitdir: ../repo/.git/worktrees/wt\n".to_string(),
            format!("gitdir: {}\n", admin.display()),
        ] {
            std::fs::write(worktree.join(".git"), &line).unwrap();
            assert_eq!(
                checked_out_branch(&worktree).as_deref(),
                Some("feature"),
                "{line}"
            );
        }
        // A `.git` file that names no git folder: none
        std::fs::write(worktree.join(".git"), "not a gitdir line\n").unwrap();
        assert_eq!(checked_out_branch(&worktree), None);
    }

    #[test]
    fn where_in_git_answers_as_git_does_and_none_outside_it() {
        let dir = crate::test_dir::TestDir::new();
        assert_eq!(super::where_in_git("--git-common-dir", dir.path()), None);
        let repo = dir.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        assert!(git(&["init", "-q"], &repo).is_some());
        let asked = super::where_in_git("--git-common-dir", &repo);
        assert_eq!(asked, git(&["rev-parse", "--git-common-dir"], &repo));
        // Asked again within the moment it is remembered: the same answer
        assert_eq!(super::where_in_git("--git-common-dir", &repo), asked);
    }

    #[test]
    fn a_failing_git_command_is_none() {
        assert_eq!(git(&["no-such-command"], &std::env::temp_dir()), None);
    }

    #[test]
    fn git_answers_with_its_output() {
        assert!(
            git(&["--version"], &std::env::temp_dir())
                .is_some_and(|v| v.starts_with("git version"))
        );
    }
}
