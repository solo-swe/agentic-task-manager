//! The history: revisions resolved, what is in `HEAD`'s history, a commit's message, `HEAD` itself.

use atm_common::Result;

use crate::{END_OF_OPTIONS, GitVcs, LITERAL, Rev, ask, run, step};

impl GitVcs {
    /// A revision's SHA, or none if there is no such commit.
    #[must_use]
    pub fn resolve(&self, rev: &Rev) -> Option<String> {
        ask(
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                END_OF_OPTIONS,
                &format!("{rev}^{{commit}}"),
            ],
            &self.root,
        )
    }

    /// The tree a revision names (a commit's, or a tree itself, as a review's rebuilt round is), or none.
    #[must_use]
    pub fn resolve_tree(&self, rev: &Rev) -> Option<String> {
        ask(
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                END_OF_OPTIONS,
                &format!("{rev}^{{tree}}"),
            ],
            &self.root,
        )
    }

    /// Whether the commit is an ancestor of HEAD (on the current branch).
    #[must_use]
    pub fn in_history(&self, sha: &Rev) -> bool {
        run(
            &[
                "merge-base",
                "--is-ancestor",
                END_OF_OPTIONS,
                sha.as_str(),
                "HEAD",
            ],
            &self.root,
        )
        .is_some_and(|o| o.status.success())
    }

    /// The commit's message.
    ///
    /// # Errors
    /// When git can't read it.
    pub fn message(&self, sha: &Rev) -> Result<String> {
        step(
            &["log", "-1", "--format=%B", END_OF_OPTIONS, sha.as_str()],
            &self.root,
            false,
        )
    }

    /// The commit that first added the file `relative` to the history of `HEAD` (the oldest, when it was removed and
    /// added again); none when the history doesn't have it, or git can't tell (ATM-630: a review's `review.md`, the
    /// commit its task's number is read at).
    #[must_use]
    pub fn first_commit_adding(&self, relative: &str) -> Option<String> {
        if relative.starts_with('-') {
            return None;
        }
        let listed = ask(
            &[
                LITERAL,
                "log",
                "--diff-filter=A",
                "--reverse",
                "--format=%H",
                "HEAD",
                "--",
                relative,
            ],
            &self.root,
        )?;
        listed
            .lines()
            .next()
            .map(str::to_string)
            .filter(|s| !s.is_empty())
    }

    /// The current commit's SHA; none outside a repository.
    #[must_use]
    pub fn head(&self) -> Option<String> {
        if self.in_repository() {
            self.resolve(&Rev::head())
        } else {
            None
        }
    }
}
