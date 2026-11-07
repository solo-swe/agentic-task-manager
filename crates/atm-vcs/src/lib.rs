//! The version-control interface and git (the Python atm's `vcs/git.py`): committing exactly the files named, the
//! history, the base branch, the remote. Git's other role, carrying the task files, is the JSON-in-git store's. Only
//! this crate and atm-db run git.
//!
//! Part of atm's Rust port (T-415): docs/porting-to-rust.md, docs/layered-architecture.md, docs/guides/rust.md.

use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output, Stdio};

use atm_common::py::{repr_str, strip};
use atm_common::{Error, Result};

/// Pathspecs that stage more than the files named: another agent's work can be in the same tree.
const WHOLE_TREE: [&str; 7] = [".", "-A", "--all", "-u", "--update", ":/", "*"];
/// git reads a path as a pattern (`*.env` is every .env file); with this, a path names exactly one file (F6).
const LITERAL: &str = "--literal-pathspecs";

/// Run git in `cwd`; none when it can't be started.
fn run(args: &[&str], cwd: &Path) -> Option<Output> {
    Command::new("git")
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .output()
        .ok()
}

/// A question for git: its output, stripped, or none when it fails.
fn ask(args: &[&str], cwd: &Path) -> Option<String> {
    let output = run(args, cwd)?;
    output
        .status
        .success()
        .then(|| strip(&String::from_utf8_lossy(&output.stdout)).to_string())
}

/// A step that must work: git's output (stripped, unless `keep` is true), or an error with git's own message.
fn step(args: &[&str], cwd: &Path, keep: bool) -> Result<String> {
    let output = run(args, cwd).ok_or_else(|| Error::failed("git isn't available"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let message = if strip(&stderr).is_empty() {
            stdout
        } else {
            stderr
        };
        let command = args
            .iter()
            .find(|a| !a.starts_with('-'))
            .copied()
            .unwrap_or_default();
        return Err(Error::failed(format!(
            "git {command} failed: {}",
            strip(&message)
        )));
    }
    Ok(if keep {
        stdout
    } else {
        strip(&stdout).to_string()
    })
}

/// `..` and `.` worked out without reading links, as Python's `normpath` does.
fn normalize(path: &Path) -> PathBuf {
    let mut normal = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                normal.pop();
            }
            Component::CurDir => {}
            other => normal.push(other),
        }
    }
    normal
}

/// The path with its links resolved, as Python's `realpath`: for one that doesn't exist, its nearest existing folder's.
fn real_path(path: &Path) -> PathBuf {
    let normal = normalize(path);
    if let Ok(real) = std::fs::canonicalize(&normal) {
        return real;
    }
    let mut missing = Vec::new();
    let mut existing = normal.as_path();
    while let Some(parent) = existing.parent() {
        missing.push(existing.file_name().map(PathBuf::from).unwrap_or_default());
        existing = parent;
        if let Ok(real) = std::fs::canonicalize(existing) {
            return missing.iter().rev().fold(real, |p, part| p.join(part));
        }
    }
    normal
}

/// `path` relative to `base`, `/`-separated, as Python's `relpath` gives it (`..` for each step up).
fn relative_to(path: &Path, base: &Path) -> String {
    let (path, base): (Vec<Component>, Vec<Component>) =
        (path.components().collect(), base.components().collect());
    let common = path.iter().zip(&base).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<String> = vec!["..".into(); base.len() - common];
    parts.extend(
        path[common..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    if parts.is_empty() {
        ".".into()
    } else {
        parts.join("/")
    }
}

/// Git as a project's version control, at its working copy's root.
#[derive(Debug, Clone)]
pub struct GitVcs {
    root: PathBuf,
}

impl GitVcs {
    /// The version control of the working copy at `root`.
    #[must_use]
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }

    /// Whether the folder is in a git repository.
    #[must_use]
    pub fn in_repository(&self) -> bool {
        ask(&["rev-parse", "--git-dir"], &self.root).is_some()
    }

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

    /// The origin remote's address, as git has it.
    #[must_use]
    pub fn remote_url(&self) -> Option<String> {
        ask(&["remote", "get-url", "origin"], &self.root)
    }

    fn known(&self, relative: &str) -> bool {
        ask(
            &[LITERAL, "ls-files", "--error-unmatch", "--", relative],
            &self.root,
        )
        .is_some()
    }

    /// The files to commit, relative to the repository, from names relative to `cwd` (none: the repository).
    ///
    /// # Errors
    /// A name that would stage more than itself, one outside the repository, a folder, a missing file git doesn't
    /// track.
    pub fn paths(&self, files: &[String], cwd: Option<&Path>) -> Result<Vec<String>> {
        let mut picked: Vec<String> = Vec::new();
        for name in files {
            if WHOLE_TREE.contains(&name.as_str()) || name.starts_with(':') {
                return Err(Error::Invalid(format!(
                    "atm vcs commit stages only the files you name; {} would stage the whole tree",
                    repr_str(name)
                )));
            }
            let full = real_path(&cwd.unwrap_or(&self.root).join(name));
            let relative = relative_to(&full, &self.root);
            if relative == "." || relative == ".." || relative.starts_with("../") {
                return Err(Error::Invalid(format!("{name} is outside the repository")));
            }
            if full.is_dir() {
                return Err(Error::Invalid(format!(
                    "{name} is a directory: name the files in it"
                )));
            }
            // A deleted file is fine as long as git knows it: still in the index (deleted by hand) or in HEAD (`git rm`)
            if !full.exists()
                && !self.known(&relative)
                && ask(&["cat-file", "-e", &format!("HEAD:{relative}")], &self.root).is_none()
            {
                return Err(Error::failed(format!(
                    "{name}: no such file, and git doesn't track it"
                )));
            }
            if !picked.contains(&relative) {
                picked.push(relative);
            }
        }
        Ok(picked)
    }

    /// `atm vcs commit --dir` (T-405): every change git sees under each folder (named relative to `cwd`), as `git
    /// status` lists it (renames with both names, new files git doesn't ignore), relative to the repository, in order.
    ///
    /// # Errors
    /// The repository's root or above, a folder outside it, the task folder, a file, a folder with nothing changed.
    pub fn changes_under(&self, folders: &[String], cwd: Option<&Path>) -> Result<Vec<String>> {
        let mut picked: Vec<String> = Vec::new();
        for name in folders {
            let full = real_path(&cwd.unwrap_or(&self.root).join(name));
            let relative = relative_to(&full, &self.root);
            if relative == "." || relative == ".." || relative.starts_with("../") {
                return Err(Error::Invalid(format!(
                    "--dir {name}: name a folder inside the repository, not the repository itself or above it"
                )));
            }
            if relative == atm_common::TASK_DIR
                || relative.starts_with(&format!("{}/", atm_common::TASK_DIR))
            {
                return Err(Error::Invalid(format!(
                    "--dir {name}: the task folder isn't committed this way (tasks.json goes with every commit)"
                )));
            }
            if full.is_file() {
                return Err(Error::Invalid(format!(
                    "--dir {name} is a file: name it with --file"
                )));
            }
            let status = step(
                &[
                    LITERAL,
                    "status",
                    "--porcelain",
                    "-z",
                    "--untracked-files=all",
                    "--",
                    &relative,
                ],
                &self.root,
                true,
            )?;
            let entries: Vec<&str> = status.split('\0').filter(|e| !e.is_empty()).collect();
            let mut found: Vec<String> = Vec::new();
            let mut i = 0;
            while i < entries.len() {
                let entry = entries[i];
                i += 1;
                if entry.len() < 4 {
                    continue;
                }
                let (code, path) = (&entry[..2], &entry[3..]);
                found.push(path.to_string());
                // A rename or a copy: the next entry is the name it had, which goes in the commit too
                if code.contains('R') || code.contains('C') {
                    if let Some(old) = entries.get(i) {
                        found.push((*old).to_string());
                    }
                    i += 1;
                }
            }
            found.retain(|p| *p == relative || p.starts_with(&format!("{relative}/")));
            if found.is_empty() {
                return Err(Error::failed(format!(
                    "--dir {name}: nothing changed under it"
                )));
            }
            for path in found {
                if !picked.contains(&path) {
                    picked.push(path);
                }
            }
        }
        Ok(picked)
    }

    /// Commit exactly `files` (relative to the repository); whatever else is staged stays staged and out of this
    /// commit. Returns the new commit's SHA.
    ///
    /// # Errors
    /// What git says when it refuses (a hook, nothing to commit).
    pub fn commit(&self, files: &[String], message: &str) -> Result<String> {
        // A deletion already staged with `git rm` is in neither the tree nor the index: there is nothing to add for it
        let to_add: Vec<&str> = files
            .iter()
            .filter(|f| self.known(f) || std::fs::symlink_metadata(self.root.join(f)).is_ok())
            .map(String::as_str)
            .collect();
        if !to_add.is_empty() {
            let mut args = vec![LITERAL, "add", "-A", "--"];
            args.extend(to_add);
            step(&args, &self.root, false)?;
        }
        let mut args = vec![LITERAL, "commit", "-q", "-m", message, "--"];
        args.extend(files.iter().map(String::as_str));
        step(&args, &self.root, false)?;
        step(&["rev-parse", "HEAD"], &self.root, false)
    }

    /// A revision's SHA, or none if there is no such commit.
    #[must_use]
    pub fn resolve(&self, rev: &str) -> Option<String> {
        ask(
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("{rev}^{{commit}}"),
            ],
            &self.root,
        )
    }

    /// Whether the commit is an ancestor of HEAD (on the current branch).
    #[must_use]
    pub fn in_history(&self, sha: &str) -> bool {
        run(&["merge-base", "--is-ancestor", sha, "HEAD"], &self.root)
            .is_some_and(|o| o.status.success())
    }

    /// The commit's message.
    ///
    /// # Errors
    /// When git can't read it.
    pub fn message(&self, sha: &str) -> Result<String> {
        step(&["log", "-1", "--format=%B", sha], &self.root, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_made_relative_as_relpath_does() {
        assert_eq!(
            relative_to(Path::new("/r/a/b.txt"), Path::new("/r")),
            "a/b.txt"
        );
        assert_eq!(relative_to(Path::new("/x"), Path::new("/r")), "../x");
        assert_eq!(relative_to(Path::new("/r"), Path::new("/r")), ".");
        assert_eq!(normalize(Path::new("/r/a/../b")), PathBuf::from("/r/b"));
    }

    #[test]
    fn whole_tree_pathspecs_are_refused() {
        let vcs = GitVcs::new(&std::env::temp_dir());
        for name in [".", "-A", ":/x"] {
            assert!(vcs.paths(&[name.to_string()], None).is_err(), "{name}");
        }
    }
}
