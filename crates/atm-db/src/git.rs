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

/// The branch checked out per a git folder's `HEAD`; a detached `HEAD` gives its short SHA, no file None.
fn head_branch(git_dir: &Path) -> Option<String> {
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
