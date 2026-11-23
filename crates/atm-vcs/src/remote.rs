//! The remote: the base branch, the origin's address, a file as the remote has it.

use crate::{GitVcs, ask, run};

impl GitVcs {
    /// The remote's default branch (`origin/HEAD`), else `main`.
    #[must_use]
    pub fn default_base(&self) -> String {
        ask(
            &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
            &self.root,
        )
        .and_then(|r| r.split_once('/').map(|(_, branch)| branch.to_string()))
        .unwrap_or_else(|| "main".into())
    }

    /// A file as it is on `origin/<branch>` (the remote-tracking branch, as last fetched), if git has it there: for
    /// task keys becoming final (ADR-007 §3), read from git, never from what an agent says.
    #[must_use]
    pub fn file_on_remote_branch(&self, branch: &str, relative: &str) -> Option<String> {
        if !atm_common::ids::valid_branch(branch) || relative.starts_with('-') {
            return None;
        }
        let output = run(
            &["show", &format!("origin/{branch}:{relative}")],
            &self.root,
        )?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// The origin remote's address, as git has it.
    #[must_use]
    pub fn remote_url(&self) -> Option<String> {
        ask(&["remote", "get-url", "origin"], &self.root)
    }
}
