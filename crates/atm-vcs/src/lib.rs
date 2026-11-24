//! The version-control interface and git (the Python atm's `vcs/git.py`): committing exactly the files named, the
//! history, the base branch, the remote. Git's other role, carrying the task files, is the JSON-in-git store's. Only
//! this crate and atm-db run git.
//!
//! Part of atm's Rust port (T-415): docs/porting-to-rust.md, docs/layered-architecture.md, docs/guides/rust.md.
//!
//! [`GitVcs`]'s methods are by concern (ATM-711): [`commit`] (staging and committing), [`history`] (revisions,
//! messages, `HEAD`), [`files`] (the working copy's and a commit's files, ignore rules, submodules) and [`remote`]
//! (the base branch, the origin). [`git`] runs git; [`paths`] works paths out as Python does.

use std::path::{Path, PathBuf};

use atm_common::{Error, Result};

mod commit;
pub mod diff;
pub use diff::{ChangedFile, Diff};
mod files;
pub use files::{ListedFile, ListedSubmodule, Listing};
mod git;
use git::{ask, run, run_fed, step};
mod history;
mod paths;
use paths::{real_path, relative_to};
mod remote;
mod rev;
pub use rev::{Rev, is_object_id};

/// After a command's options, before its revisions: git reads nothing after it as an option, a second guard beside
/// [`Rev`]'s check (ATM-659).
const END_OF_OPTIONS: &str = "--end-of-options";

/// Pathspecs that stage more than the files named: another agent's work can be in the same tree.
const WHOLE_TREE: [&str; 7] = [".", "-A", "--all", "-u", "--update", ":/", "*"];
/// git reads a path as a pattern (`*.env` is every .env file); with this, a path names exactly one file (F6).
const LITERAL: &str = "--literal-pathspecs";

/// The version controls atm knows, by the name a project's settings file gives them (ADR-003 §4).
pub const VCS_KINDS: [&str; 1] = ["git"];

/// The version control `kind` (a project's settings say which) of the working copy at `root`.
///
/// # Errors
/// A kind atm doesn't know.
pub fn open(root: &Path, kind: &str) -> Result<GitVcs> {
    if !VCS_KINDS.contains(&kind) {
        return Err(Error::Invalid(format!(
            "the version control {kind:?} isn't one atm knows ({})",
            VCS_KINDS.join(", ")
        )));
    }
    Ok(GitVcs::new(root))
}

/// Git as a project's version control, at its working copy's root.
#[derive(Debug, Clone)]
pub struct GitVcs {
    root: PathBuf,
    /// For a submodule's repository opened from the project (ATM-583): its folder from the project's root, with a
    /// trailing `/` (`lib/`), which its diffs show paths under and its patches are applied without; else empty.
    shown: String,
}

impl GitVcs {
    /// The version control of the working copy at `root`.
    #[must_use]
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            shown: String::new(),
        }
    }

    /// The repository of the submodule at `folder` (from this root, `/`-separated), its diffs showing paths as the
    /// project has them (`lib/b.txt`), so a review's round of commits made in it reads like any other (ATM-583). None
    /// when `folder` isn't a submodule checked out here (a nested submodule's folder included: name the outer one).
    #[must_use]
    pub fn submodule_at(&self, folder: &str) -> Option<GitVcs> {
        let folder = folder.trim_end_matches('/');
        let (found, _) = self.submodule(&format!("{folder}/-"))?;
        (found == folder).then(|| GitVcs {
            root: self.root.join(folder),
            shown: format!("{}{folder}/", self.shown),
        })
    }

    /// The folder (from this root) of the submodule that `dir` is in, nearest the root; none when it is in none.
    #[must_use]
    pub fn submodule_containing(&self, dir: &Path) -> Option<String> {
        let relative = relative_to(&real_path(dir), &real_path(&self.root));
        if relative == "." || relative.starts_with("..") {
            return None;
        }
        self.submodule(&format!("{relative}/-"))
            .map(|(folder, _)| folder)
    }

    /// The folder of the submodule this repository is, from the project's root (`lib`), or none for the project's own.
    #[must_use]
    pub fn shown_folder(&self) -> Option<&str> {
        (!self.shown.is_empty()).then(|| self.shown.trim_end_matches('/'))
    }

    /// A path as the project has it, from one as this repository has it.
    fn shown_path(&self, inside: &str) -> String {
        format!("{}{inside}", self.shown)
    }

    /// A path as this repository has it, from one as the project has it; none for a path outside it.
    fn inside_path<'a>(&self, shown: &'a str) -> Option<&'a str> {
        shown.strip_prefix(self.shown.as_str())
    }

    /// Whether the folder is in a git repository.
    #[must_use]
    pub fn in_repository(&self) -> bool {
        ask(&["rev-parse", "--git-dir"], &self.root).is_some()
    }
}

#[cfg(test)]
mod tests;
