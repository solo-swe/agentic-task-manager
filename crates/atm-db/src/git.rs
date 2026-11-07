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

#[cfg(test)]
mod tests {
    use super::git;

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
