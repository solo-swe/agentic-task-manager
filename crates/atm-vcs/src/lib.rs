//! The version-control interface and git (the Python atm's `vcs/git.py`): committing exactly the files named, the
//! history, the base branch, the remote. Git's other role, carrying the task files, is the JSON-in-git store's. Only
//! this crate and atm-db run git.
//!
//! Part of atm's Rust port (T-415): docs/porting-to-rust.md, docs/layered-architecture.md, docs/guides/rust.md.

use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output, Stdio};

use atm_common::py::{repr_str, strip};
use atm_common::{Error, Result};

pub mod diff;
pub use diff::{ChangedFile, Diff};

/// Pathspecs that stage more than the files named: another agent's work can be in the same tree.
const WHOLE_TREE: [&str; 7] = [".", "-A", "--all", "-u", "--update", ":/", "*"];
/// git reads a path as a pattern (`*.env` is every .env file); with this, a path names exactly one file (F6).
const LITERAL: &str = "--literal-pathspecs";

/// Run git in `cwd`; none when it can't be started.
fn run(args: &[&str], cwd: &Path) -> Option<Output> {
    run_fed(args, cwd, None)
}

/// Run git in `cwd`, with `input` on its stdin when given (paths for `--stdin`, never as arguments); none when it
/// can't be started or fed. The one place this crate starts git (I24).
fn run_fed(args: &[&str], cwd: &Path, input: Option<&[u8]>) -> Option<Output> {
    use std::io::Write as _;
    let mut command = Command::new("git");
    command.args(args).current_dir(cwd);
    let Some(input) = input else {
        return command.stdin(Stdio::null()).output().ok();
    };
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdin = child.stdin.take()?;
    stdin.write_all(input).ok()?;
    drop(stdin);
    child.wait_with_output().ok()
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
    // atm-common's `canonical`: on Windows without `\\?\`, so it compares with the root
    let normal = normalize(path);
    if let Ok(real) = atm_common::pathtext::canonical(&normal) {
        return real;
    }
    let mut missing = Vec::new();
    let mut existing = normal.as_path();
    while let Some(parent) = existing.parent() {
        missing.push(existing.file_name().map(PathBuf::from).unwrap_or_default());
        existing = parent;
        if let Ok(real) = atm_common::pathtext::canonical(existing) {
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

    /// The tree a revision names (a commit's, or a tree itself, as a review's rebuilt round is), or none.
    #[must_use]
    pub fn resolve_tree(&self, rev: &str) -> Option<String> {
        ask(
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("{rev}^{{tree}}"),
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

    // The working copy's files, for the document viewer (docs/compmd-viewer-design.md §4). atm-docs checks the path
    // first; these read only below the root, through no symbolic link

    /// The file's path, or none when a segment from the root down is a symbolic link or it isn't a regular file.
    fn file(&self, relative: &str) -> Option<PathBuf> {
        let mut current = self.root.clone();
        for segment in relative.split('/') {
            current.push(segment);
            if std::fs::symlink_metadata(&current).is_ok_and(|m| m.file_type().is_symlink()) {
                return None;
            }
        }
        current.is_file().then_some(current)
    }

    /// `(size, nanosecond mtime)` of a file of the working copy, or none when it isn't one atm reads.
    #[must_use]
    pub fn file_stamp(&self, relative: &str) -> Option<(u64, u128)> {
        let meta = std::fs::metadata(self.file(relative)?).ok()?;
        let mtime = meta
            .modified()
            .ok()?
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_nanos();
        Some((meta.len(), mtime))
    }

    /// The bytes of a file of the working copy.
    ///
    /// # Errors
    /// `NotFound` when it isn't one atm reads, or can't be read.
    pub fn read_file(&self, relative: &str) -> Result<Vec<u8>> {
        self.file(relative)
            .and_then(|p| std::fs::read(p).ok())
            .ok_or_else(|| Error::NotFound(format!("{relative}: no such file")))
    }

    /// A regular file as `commit` (a SHA, as [`GitVcs::resolve`] gives it) has it: its size and its blob; none when the
    /// commit has no regular file there (no entry, a symlink, a submodule, a folder).
    #[must_use]
    pub fn blob_in(&self, commit: &str, relative: &str) -> Option<(u64, String)> {
        let output = run(
            &["ls-tree", "-z", "--long", commit, "--", relative],
            &self.root,
        )?;
        if !output.status.success() {
            return None;
        }
        let listing = String::from_utf8_lossy(&output.stdout);
        let (meta, path) = listing.split('\0').next()?.split_once('\t')?;
        if path != relative {
            return None;
        }
        let parts: Vec<&str> = meta.split_whitespace().collect();
        let [mode, "blob", blob, size] = parts.as_slice() else {
            return None;
        };
        matches!(*mode, "100644" | "100755").then_some(())?;
        Some((size.parse().ok()?, (*blob).to_string()))
    }

    /// The commit a submodule is at in `object` (a commit or a tree): its entry at `relative` is a link (mode
    /// 160000); none for anything else there, or nothing (ATM-599: a submodule only moved has no other trace in a
    /// diff).
    #[must_use]
    pub fn gitlink_in(&self, object: &str, relative: &str) -> Option<String> {
        // Named as the project has it; read as this repository has it (a submodule's round, ATM-583)
        let relative = self.inside_path(relative)?;
        let output = run(&["ls-tree", "-z", object, "--", relative], &self.root)?;
        if !output.status.success() {
            return None;
        }
        let listing = String::from_utf8_lossy(&output.stdout);
        let (meta, path) = listing.split('\0').next()?.split_once('\t')?;
        let parts: Vec<&str> = meta.split_whitespace().collect();
        match parts.as_slice() {
            ["160000", "commit", sha] if path == relative => Some((*sha).to_string()),
            _ => None,
        }
    }

    /// A blob's bytes (from [`GitVcs::blob_in`]).
    ///
    /// # Errors
    /// git isn't available, or there is no such blob.
    pub fn blob_bytes(&self, blob: &str) -> Result<Vec<u8>> {
        let output = run(&["cat-file", "blob", blob], &self.root)
            .ok_or_else(|| Error::failed("git isn't available"))?;
        if !output.status.success() {
            return Err(Error::NotFound(format!("{blob}: no such blob")));
        }
        Ok(output.stdout)
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

    /// The submodule `relative` is in, nearest the root, as (its folder, the path inside it), or none. A submodule is a
    /// folder this repository records as a commit (mode 160000) with a checkout of its own (`.git`); git answers
    /// nothing about the files in it from here, only from its own repository (T-408).
    fn submodule(&self, relative: &str) -> Option<(String, String)> {
        let parts: Vec<&str> = relative.split('/').collect();
        for i in 1..parts.len() {
            let folder = parts[..i].join("/");
            if std::fs::symlink_metadata(self.root.join(&folder).join(".git")).is_err() {
                continue;
            }
            let listed = ask(&[LITERAL, "ls-files", "--stage", "--", &folder], &self.root)
                .unwrap_or_default();
            for line in listed.lines() {
                let (meta, path) = line.split_once('\t').unwrap_or((line, ""));
                if path == folder && meta.split(' ').next() == Some("160000") {
                    return Some((folder, parts[i..].join("/")));
                }
            }
        }
        None
    }

    /// The submodule's own version control and the path inside it, or none outside one.
    #[must_use]
    pub fn in_submodule(&self, relative: &str) -> Option<(GitVcs, String)> {
        self.submodule(relative)
            .map(|(folder, inside)| (GitVcs::new(&self.root.join(folder)), inside))
    }

    /// git's ignore rules, as `git status` applies them: a file git tracks is never ignored. It fails closed: only
    /// git's "not ignored" (exit 1) lets a file through; an error counts as ignored. A file in a submodule: the
    /// submodule's own rules decide, down to the innermost submodule.
    #[must_use]
    pub fn ignored(&self, relative: &str) -> bool {
        if !self.in_repository() {
            return false;
        }
        if let Some((sub, inside)) = self.in_submodule(relative) {
            return sub.ignored(&inside);
        }
        run(&["check-ignore", "-q", "--", relative], &self.root)
            .is_none_or(|o| o.status.code() != Some(1))
    }

    /// The working copy's state as a live review shows it (ATM-607), cheap to make: the commit checked out, and each
    /// changed file outside atm's task folder (`git status`, untracked files too, ignored ones never) with its status,
    /// size and modification time. The same text while nothing changes; a page's `ETag` is made from it. None outside
    /// a repository, or when git can't say.
    #[must_use]
    pub fn working_state(&self) -> Option<String> {
        let head = ask(&["rev-parse", "--verify", "-q", "HEAD"], &self.root).unwrap_or_default();
        let status = run(
            &["status", "--porcelain", "-z", "--untracked-files=all"],
            &self.root,
        )
        .filter(|o| o.status.success())?;
        let status = String::from_utf8_lossy(&status.stdout).into_owned();
        let task_folder = format!("{}/", atm_common::TASK_DIR);
        let mut state = vec![head];
        for entry in status.split('\0').filter(|e| e.len() >= 4) {
            let path = &entry[3..];
            if path.starts_with(&task_folder) {
                continue;
            }
            let version = std::fs::metadata(self.root.join(path)).ok().map(|m| {
                let modified = m
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_nanos());
                format!("{}:{modified}", m.len())
            });
            state.push(format!("{entry}\t{}", version.unwrap_or_default()));
        }
        Some(state.join("\n"))
    }

    /// Whether any of `relative` is a file git doesn't ignore, in one git run for them all (`check-ignore --stdin`): a
    /// burst of changes in a working copy, which a build or an install makes of thousands of ignored files (ATM-607).
    /// Files in a submodule are judged by the project's rules here. Outside a repository every file counts; when git
    /// can't answer, the burst counts too (a page asks again; it never misses a change).
    #[must_use]
    pub fn any_not_ignored(&self, relative: &[String]) -> bool {
        if relative.is_empty() {
            return false;
        }
        if !self.in_repository() {
            return true;
        }
        let input: Vec<u8> = relative
            .iter()
            .flat_map(|p| p.bytes().chain(std::iter::once(0)))
            .collect();
        let Some(output) = run_fed(&["check-ignore", "--stdin", "-z"], &self.root, Some(&input))
        else {
            return true;
        };
        // Exit 0: some are ignored, listed; 1, none is; anything else, git couldn't say (both of those count)
        if output.status.code() != Some(0) {
            return true;
        }
        let ignored: std::collections::HashSet<&[u8]> = output
            .stdout
            .split(|b| *b == 0)
            .filter(|p| !p.is_empty())
            .collect();
        relative.iter().any(|p| !ignored.contains(p.as_bytes()))
    }

    /// Whether git tracks the file; none outside a repository.
    #[must_use]
    pub fn tracked(&self, relative: &str) -> Option<bool> {
        if !self.in_repository() {
            return None;
        }
        if let Some((sub, inside)) = self.in_submodule(relative) {
            return sub.tracked(&inside);
        }
        Some(self.known(relative))
    }

    /// The current commit's SHA; none outside a repository.
    #[must_use]
    pub fn head(&self) -> Option<String> {
        if self.in_repository() {
            self.resolve("HEAD")
        } else {
            None
        }
    }

    /// For a file in a submodule (the innermost): `{path: its folder from the root, commit: its HEAD, repository: its
    /// origin's web address or null}`; none for any other file.
    #[must_use]
    pub fn submodule_of(&self, relative: &str) -> Option<serde_json::Value> {
        if !self.in_repository() {
            return None;
        }
        let (folder, inside) = self.submodule(relative)?;
        let sub = GitVcs::new(&self.root.join(&folder));
        if let Some(mut nested) = sub.submodule_of(&inside) {
            let path = format!("{folder}/{}", nested["path"].as_str().unwrap_or_default());
            nested["path"] = serde_json::json!(path);
            return Some(nested);
        }
        let repository = sub
            .remote_url()
            .and_then(|r| atm_common::repository::repository_url(&r));
        Some(serde_json::json!({"path": folder, "commit": sub.head(), "repository": repository}))
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
        for name in WHOLE_TREE.iter().copied().chain([":/x", ":(glob)*.rs"]) {
            let error = vcs.paths(&[name.to_string()], None).unwrap_err();
            assert!(
                matches!(&error, Error::Invalid(m) if m.ends_with("would stage the whole tree")),
                "{name}: {error:?}"
            );
        }
    }

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
        assert_eq!(vcs.resolve("HEAD"), None);
    }

    #[test]
    fn the_files_to_commit_are_named_relative_to_here_once_each() {
        let dir = scratch();
        let vcs = init(&dir.0);
        write(&vcs, "src/a.rs", "a");
        write(&vcs, "b.txt", "b");
        let picked = vcs
            .paths(
                &names(&["a.rs", "../b.txt", "./a.rs", "../src/a.rs"]),
                Some(&dir.0.join("src")),
            )
            .unwrap();
        assert_eq!(picked, ["src/a.rs", "b.txt"]);
        assert_eq!(vcs.paths(&[], None).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn a_name_outside_the_repository_a_folder_or_a_file_git_doesnt_know_is_refused() {
        let dir = scratch();
        let vcs = init(&dir.0.join("repo"));
        write(&vcs, "src/a.rs", "a");
        std::fs::write(dir.0.join("outside.txt"), "x").unwrap();
        // (the name, the message)
        for (name, message) in [
            ("../outside.txt", "../outside.txt is outside the repository"),
            ("..", ".. is outside the repository"),
            ("src/..", "src/.. is outside the repository"),
            ("src", "src is a directory: name the files in it"),
            (
                "missing.txt",
                "missing.txt: no such file, and git doesn't track it",
            ),
        ] {
            let error = vcs.paths(&names(&[name]), None).unwrap_err();
            assert_eq!(error.to_string(), message, "{name}");
        }
    }

    #[test]
    fn a_deleted_file_git_knows_can_be_named() {
        let dir = scratch();
        let vcs = init(&dir.0);
        write(&vcs, "by-hand.txt", "a");
        write(&vcs, "git-rm.txt", "b");
        commit_all(&vcs, "first");
        std::fs::remove_file(dir.0.join("by-hand.txt")).unwrap();
        git(&dir.0, &["rm", "-q", "git-rm.txt"]);
        assert_eq!(
            vcs.paths(&names(&["by-hand.txt", "git-rm.txt"]), None)
                .unwrap(),
            ["by-hand.txt", "git-rm.txt"]
        );
    }

    #[test]
    fn a_name_is_a_literal_path_never_a_pattern() {
        let dir = scratch();
        let vcs = init(&dir.0);
        write(&vcs, "a.env", "secret");
        write(&vcs, "b.env", "secret");
        commit_all(&vcs, "first");
        write(&vcs, "a.env", "changed");
        write(&vcs, "b.env", "changed");
        // `*.env` names no file: refused, not every .env file
        let error = vcs.paths(&names(&["*.env"]), None).unwrap_err();
        assert_eq!(
            error.to_string(),
            "*.env: no such file, and git doesn't track it"
        );
        vcs.commit(&names(&["a.env"]), "only a").unwrap();
        let status = git(&dir.0, &["status", "--porcelain"]);
        assert_eq!(status, "M b.env");
    }

    #[test]
    fn a_commit_holds_exactly_the_files_named_and_what_else_is_staged_stays_staged() {
        let dir = scratch();
        let vcs = init(&dir.0);
        write(&vcs, "a.txt", "a");
        write(&vcs, "gone.txt", "g");
        let first = commit_all(&vcs, "first");
        write(&vcs, "a.txt", "a2");
        write(&vcs, "new.txt", "n");
        write(&vcs, "staged.txt", "s");
        git(&dir.0, &["add", "staged.txt"]);
        git(&dir.0, &["rm", "-q", "gone.txt"]);
        let sha = vcs
            .commit(&names(&["a.txt", "new.txt", "gone.txt"]), "the change")
            .unwrap();
        assert_eq!(vcs.head().as_deref(), Some(sha.as_str()));
        assert_eq!(vcs.resolve("HEAD"), Some(sha.clone()));
        assert_eq!(vcs.message(&sha).unwrap(), "the change");
        let changed = git(&dir.0, &["show", "--name-status", "--format=", &sha]);
        assert_eq!(changed, "M\ta.txt\nD\tgone.txt\nA\tnew.txt");
        assert_eq!(git(&dir.0, &["status", "--porcelain"]), "A  staged.txt");
        assert!(vcs.in_history(&first) && vcs.in_history(&sha));
    }

    #[test]
    fn a_commit_git_refuses_says_what_git_said() {
        let dir = scratch();
        let vcs = init(&dir.0);
        write(&vcs, "a.txt", "a");
        commit_all(&vcs, "first");
        let error = vcs.commit(&names(&["a.txt"]), "nothing").unwrap_err();
        assert!(
            error.to_string().starts_with("git commit failed: "),
            "{error}"
        );
        assert!(
            vcs.message("no-such-commit")
                .unwrap_err()
                .to_string()
                .starts_with("git log failed: ")
        );
    }

    #[test]
    fn revisions_resolve_to_their_commit_or_tree_and_only_ancestors_are_in_history() {
        let dir = scratch();
        let vcs = init(&dir.0);
        write(&vcs, "a.txt", "a");
        let first = commit_all(&vcs, "first");
        let tree = git(&dir.0, &["rev-parse", "HEAD^{tree}"]);
        assert_eq!(vcs.resolve("main"), Some(first.clone()));
        assert_eq!(vcs.resolve_tree(&first), Some(tree.clone()));
        assert_eq!(
            vcs.resolve_tree(&tree),
            Some(tree.clone()),
            "a tree is its own"
        );
        assert_eq!(vcs.resolve(&tree), None, "a tree is no commit");
        assert_eq!(vcs.resolve("no-such-branch"), None);
        git(&dir.0, &["checkout", "-q", "-b", "side"]);
        write(&vcs, "b.txt", "b");
        let side = commit_all(&vcs, "side");
        git(&dir.0, &["checkout", "-q", "main"]);
        assert!(vcs.in_history(&first));
        assert!(!vcs.in_history(&side), "on another branch");
        assert!(!vcs.in_history("no-such-commit"));
    }

    #[test]
    fn changes_under_a_folder_are_what_git_status_lists_there_renames_with_both_names() {
        let dir = scratch();
        let vcs = init(&dir.0);
        write(
            &vcs,
            "docs/old.md",
            "same text, long enough to be seen as a rename\n",
        );
        write(&vcs, "docs/kept.md", "k");
        write(&vcs, "docs-other/x.md", "x");
        write(&vcs, ".gitignore", "*.log\n");
        commit_all(&vcs, "first");
        git(&dir.0, &["mv", "docs/old.md", "docs/new.md"]);
        write(&vcs, "docs/kept.md", "k2");
        write(&vcs, "docs/deep/added.md", "a");
        write(&vcs, "docs/build.log", "ignored");
        write(&vcs, "docs-other/x.md", "x2");
        let mut found = vcs.changes_under(&names(&["docs"]), None).unwrap();
        found.sort();
        assert_eq!(
            found,
            [
                "docs/deep/added.md",
                "docs/kept.md",
                "docs/new.md",
                "docs/old.md"
            ]
        );
        // From a folder inside, and two folders, each change once
        let from_inside = vcs
            .changes_under(
                &names(&["deep", "../docs-other", "deep"]),
                Some(&dir.0.join("docs")),
            )
            .unwrap();
        assert_eq!(from_inside, ["docs/deep/added.md", "docs-other/x.md"]);
    }

    #[test]
    fn changes_under_refuse_the_root_above_it_the_task_folder_a_file_and_a_clean_folder() {
        let dir = scratch();
        let vcs = init(&dir.0.join("repo"));
        write(&vcs, "clean/a.txt", "a");
        write(&vcs, &format!("{}/tasks.json", atm_common::TASK_DIR), "{}");
        commit_all(&vcs, "first");
        write(
            &vcs,
            &format!("{}/tasks.json", atm_common::TASK_DIR),
            "{\"tasks\": []}",
        );
        // (the folder, the message's start)
        for (name, message) in [
            (".", "--dir .: name a folder inside the repository"),
            ("..", "--dir ..: name a folder inside the repository"),
            (
                atm_common::TASK_DIR,
                "--dir .agentic-task-manager: the task folder isn't committed this way",
            ),
            (
                "clean/a.txt",
                "--dir clean/a.txt is a file: name it with --file",
            ),
            ("clean", "--dir clean: nothing changed under it"),
            ("missing", "--dir missing: nothing changed under it"),
        ] {
            let error = vcs.changes_under(&names(&[name]), None).unwrap_err();
            assert!(error.to_string().starts_with(message), "{name}: {error}");
        }
    }

    #[test]
    fn a_working_copy_file_is_read_only_below_the_root_through_no_link() {
        let dir = scratch();
        let vcs = init(&dir.0.join("repo"));
        write(&vcs, "docs/a.md", "hello");
        assert_eq!(vcs.read_file("docs/a.md").unwrap(), b"hello");
        let (size, mtime) = vcs.file_stamp("docs/a.md").unwrap();
        assert_eq!(size, 5);
        assert!(mtime > 0);
        for missing in ["docs/none.md", "docs", ""] {
            assert_eq!(vcs.file_stamp(missing), None, "{missing:?}");
            assert_eq!(
                vcs.read_file(missing).unwrap_err(),
                Error::NotFound(format!("{missing}: no such file"))
            );
        }
        #[cfg(unix)]
        {
            std::fs::write(dir.0.join("outside.md"), "secret").unwrap();
            std::os::unix::fs::symlink(dir.0.join("outside.md"), vcs.root.join("link.md")).unwrap();
            std::os::unix::fs::symlink(vcs.root.join("docs"), vcs.root.join("linked")).unwrap();
            for linked in ["link.md", "linked/a.md"] {
                assert_eq!(vcs.file_stamp(linked), None, "{linked}");
                assert!(
                    matches!(vcs.read_file(linked), Err(Error::NotFound(_))),
                    "{linked}"
                );
            }
        }
    }

    #[test]
    fn a_commits_regular_file_has_its_size_and_blob_and_anything_else_none() {
        let dir = scratch();
        let vcs = init(&dir.0);
        write(&vcs, "docs/a.md", "hello");
        write(&vcs, "run.sh", "#!/bin/sh\n");
        git(&dir.0, &["add", "-A"]);
        git(&dir.0, &["update-index", "--chmod=+x", "run.sh"]);
        #[cfg(unix)]
        std::os::unix::fs::symlink("docs/a.md", vcs.root.join("link.md")).unwrap();
        let head = commit_all(&vcs, "first");
        let (size, blob) = vcs.blob_in(&head, "docs/a.md").unwrap();
        assert_eq!(size, 5);
        assert_eq!(vcs.blob_bytes(&blob).unwrap(), b"hello");
        assert_eq!(
            vcs.blob_in(&head, "run.sh").map(|(s, _)| s),
            Some(10),
            "executable"
        );
        for other in ["docs", "missing.md", "docs/a", "link.md"] {
            assert_eq!(vcs.blob_in(&head, other), None, "{other}");
        }
        assert_eq!(vcs.blob_in("no-such-commit", "docs/a.md"), None);
        let unknown = "0".repeat(40);
        assert_eq!(
            vcs.blob_bytes(&unknown).unwrap_err(),
            Error::NotFound(format!("{unknown}: no such blob"))
        );
    }

    #[test]
    fn the_commit_first_adding_a_file_is_the_oldest_in_heads_history() {
        let dir = scratch();
        let vcs = init(&dir.0);
        assert_eq!(vcs.first_commit_adding("a.md"), None, "no commit yet");
        write(&vcs, "a.md", "one");
        let added = commit_all(&vcs, "add");
        write(&vcs, "a.md", "two");
        commit_all(&vcs, "change");
        assert_eq!(vcs.first_commit_adding("a.md"), Some(added.clone()));
        // Removed and added again: still the first
        git(&dir.0, &["rm", "-q", "a.md"]);
        commit_all(&vcs, "remove");
        write(&vcs, "a.md", "three");
        commit_all(&vcs, "again");
        assert_eq!(vcs.first_commit_adding("a.md"), Some(added));
        for other in ["missing.md", "-a.md", "*.md"] {
            assert_eq!(vcs.first_commit_adding(other), None, "{other}");
        }
    }

    #[test]
    fn a_tracked_file_is_never_ignored_and_ignore_rules_decide_the_rest() {
        let dir = scratch();
        let vcs = init(&dir.0);
        write(&vcs, ".gitignore", "*.log\n");
        write(&vcs, "kept.log", "tracked before the rule mattered");
        git(&dir.0, &["add", "-f", "kept.log", ".gitignore"]);
        git(&dir.0, &["commit", "-q", "-m", "first"]);
        write(&vcs, "new.log", "n");
        write(&vcs, "new.txt", "n");
        // (the file, ignored, tracked)
        for (file, ignored, tracked) in [
            ("kept.log", false, true),
            ("new.log", true, false),
            ("new.txt", false, false),
            ("missing.txt", false, false),
        ] {
            assert_eq!(vcs.ignored(file), ignored, "{file}");
            assert_eq!(vcs.tracked(file), Some(tracked), "{file}");
        }
    }

    /// ATM-607: a burst of changes counts when any file in it is one git doesn't ignore, in one git run
    /// ATM-607: the working copy's state changes with a file of the work written, and only then
    #[test]
    fn the_working_state_changes_with_the_work_only() {
        let dir = scratch();
        let vcs = init(&dir.0);
        write(&vcs, ".gitignore", "*.log\n");
        write(&vcs, "a.txt", "one\n");
        git(&dir.0, &["add", "."]);
        git(&dir.0, &["commit", "-q", "-m", "first"]);
        let clean = vcs.working_state().unwrap();
        assert_eq!(vcs.working_state().unwrap(), clean, "nothing changed");
        write(&vcs, "build.log", "ignored");
        write(&vcs, ".agentic-task-manager/tasks.json", "{}");
        assert_eq!(
            vcs.working_state().unwrap(),
            clean,
            "an ignored file, or atm's folder"
        );
        write(&vcs, "a.txt", "two\n");
        let edited = vcs.working_state().unwrap();
        assert_ne!(edited, clean, "a file of the work edited");
        std::thread::sleep(std::time::Duration::from_millis(20));
        write(&vcs, "a.txt", "three\n");
        assert_ne!(vcs.working_state().unwrap(), edited, "edited again");
        write(&vcs, "new.txt", "n");
        assert!(
            vcs.working_state().unwrap().contains("new.txt"),
            "a new file"
        );
        assert_eq!(GitVcs::new(&dir.0.join("not-a-repo")).working_state(), None);
    }

    #[test]
    fn a_burst_counts_when_any_of_its_files_isnt_ignored() {
        let dir = scratch();
        let vcs = init(&dir.0);
        write(&vcs, ".gitignore", "*.log\ntarget/\n");
        write(&vcs, "kept.log", "tracked");
        git(&dir.0, &["add", "-f", "kept.log", ".gitignore"]);
        git(&dir.0, &["commit", "-q", "-m", "first"]);
        let burst = |paths: &[&str]| {
            vcs.any_not_ignored(&paths.iter().map(|p| (*p).to_string()).collect::<Vec<_>>())
        };
        assert!(!burst(&[]), "nothing changed");
        assert!(
            !burst(&["a.log", "target/debug/atm", "target/x"]),
            "all ignored"
        );
        assert!(burst(&["a.log", "src/main.rs"]), "one that isn't");
        assert!(
            burst(&["kept.log"]),
            "a tracked file, though its name is ignored"
        );
        assert!(burst(&["new.txt"]), "a new file, not ignored");
        assert!(
            GitVcs::new(&dir.0.join("not-a-repo")).any_not_ignored(&["a.log".into()]),
            "outside a repository every file counts"
        );
    }

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

    /// ATM-599: a submodule's link in a commit or a tree gives its commit; a file, a folder or nothing there, none
    #[test]
    fn a_submodules_link_in_a_commit_or_tree_names_its_commit() {
        let dir = scratch();
        let sub = init(&dir.0.join("sub-origin"));
        write(&sub, "a.txt", "a");
        let sub_head = commit_all(&sub, "sub");
        let vcs = init(&dir.0.join("repo"));
        write(&vcs, "lib/top.txt", "t");
        commit_all(&vcs, "first");
        git(
            &vcs.root,
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "--quiet",
                "add",
                "../sub-origin",
                "lib/sub",
            ],
        );
        let head = commit_all(&vcs, "the submodule");
        let tree = vcs.resolve_tree(&head).unwrap();
        for object in [head.as_str(), tree.as_str()] {
            assert_eq!(
                vcs.gitlink_in(object, "lib/sub"),
                Some(sub_head.clone()),
                "{object}"
            );
            for other in ["lib/top.txt", "lib", "lib/none", "lib/sub/a.txt", "-p"] {
                assert_eq!(vcs.gitlink_in(object, other), None, "{object} {other}");
            }
        }
    }

    #[test]
    fn a_file_in_a_submodule_is_answered_by_the_submodules_own_repository() {
        let dir = scratch();
        let sub = init(&dir.0.join("sub-origin"));
        write(&sub, "a.txt", "a");
        write(&sub, ".gitignore", "*.tmp\n");
        let sub_head = commit_all(&sub, "sub");
        let vcs = init(&dir.0.join("repo"));
        write(&vcs, "top.txt", "t");
        commit_all(&vcs, "first");
        git(
            &vcs.root,
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "--quiet",
                "add",
                "../sub-origin",
                "lib/sub",
            ],
        );
        commit_all(&vcs, "the submodule");
        write(&vcs, "lib/sub/new.tmp", "n");
        assert_eq!(
            vcs.tracked("lib/sub/a.txt"),
            Some(true),
            "tracked by the submodule"
        );
        assert_eq!(vcs.tracked("lib/sub/new.tmp"), Some(false));
        assert!(vcs.ignored("lib/sub/new.tmp"), "the submodule's rules");
        assert!(!vcs.ignored("lib/sub/a.txt"));
        let of = vcs.submodule_of("lib/sub/a.txt").unwrap();
        assert_eq!(of["path"], serde_json::json!("lib/sub"));
        assert_eq!(of["commit"], serde_json::json!(sub_head));
        assert_eq!(vcs.submodule_of("top.txt"), None);
        // A folder with a `.git` that this repository doesn't record as a submodule is no submodule
        let nested = init(&dir.0.join("repo/nested"));
        write(&nested, "b.txt", "b");
        assert_eq!(vcs.submodule_of("nested/b.txt"), None);
    }
}
