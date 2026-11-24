//! Files: the working copy's (for the document viewer) and a commit's, ignore rules and tracking, the working state,
//! and submodules.

use std::path::PathBuf;

use atm_common::{Error, Result};

use crate::{END_OF_OPTIONS, GitVcs, LITERAL, Rev, ask, run, run_fed};

impl GitVcs {
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

    /// The first `limit` bytes of a file of the working copy (fewer when it is shorter), as [`GitVcs::read_file`] reads
    /// it: a listing's titles read no more of a large file (ATM-764). None when it isn't one atm reads.
    #[must_use]
    pub fn read_file_start(&self, relative: &str, limit: usize) -> Option<Vec<u8>> {
        use std::io::Read as _;
        let file = std::fs::File::open(self.file(relative)?).ok()?;
        let mut start = Vec::with_capacity(limit.min(64 * 1024));
        file.take(limit as u64).read_to_end(&mut start).ok()?;
        Some(start)
    }

    /// A regular file as `commit` (a SHA, as [`GitVcs::resolve`] gives it) has it: its size and its blob; none when the
    /// commit has no regular file there (no entry, a symlink, a submodule, a folder).
    #[must_use]
    pub fn blob_in(&self, commit: &Rev, relative: &str) -> Option<(u64, String)> {
        let output = run(
            &[
                "ls-tree",
                "-z",
                "--long",
                END_OF_OPTIONS,
                commit.as_str(),
                "--",
                relative,
            ],
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
    pub fn gitlink_in(&self, object: &Rev, relative: &str) -> Option<String> {
        // Named as the project has it; read as this repository has it (a submodule's round, ATM-583)
        let relative = self.inside_path(relative)?;
        let output = run(
            &[
                "ls-tree",
                "-z",
                END_OF_OPTIONS,
                object.as_str(),
                "--",
                relative,
            ],
            &self.root,
        )?;
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
    pub fn blob_bytes(&self, blob: &Rev) -> Result<Vec<u8>> {
        let output = run(
            &["cat-file", "blob", END_OF_OPTIONS, blob.as_str()],
            &self.root,
        )
        .ok_or_else(|| Error::failed("git isn't available"))?;
        if !output.status.success() {
            return Err(Error::NotFound(format!("{blob}: no such blob")));
        }
        Ok(output.stdout)
    }

    /// The submodule `relative` is in, nearest the root, as (its folder, the path inside it), or none. A submodule is a
    /// folder this repository records as a commit (mode 160000) with a checkout of its own (`.git`); git answers
    /// nothing about the files in it from here, only from its own repository (T-408).
    pub(crate) fn submodule(&self, relative: &str) -> Option<(String, String)> {
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
        let mut records = status.split('\0');
        while let Some(entry) = records.next() {
            // `XY path`; a rename or a copy (R or C in either column) has its old path as the next field, with no
            // status before it (ATM-670, audit H1: three bytes were cut off it too, `README.md` read as `DME.md`, and a
            // multibyte first character panicked)
            let (Some(status), Some(path)) = (entry.get(..2), entry.get(3..)) else {
                continue;
            };
            let from = if status.contains(['R', 'C']) {
                records.next()
            } else {
                None
            };
            if path.is_empty() || path.starts_with(&task_folder) {
                continue;
            }
            let entry = match from {
                Some(from) => format!("{entry}\0{from}"),
                None => entry.to_string(),
            };
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

    /// The working copy's files, as git has them (ATM-764, docs/documents-design.md §2): those it tracks (`ls-files`)
    /// and those untracked it doesn't ignore (`ls-files --others --exclude-standard`, by this repository's own rules),
    /// each by its path from this root; and the submodules it records (mode 160000), each saying whether it is checked
    /// out here (its folder has a checkout of its own). Nothing inside a submodule: list it from its own repository
    /// ([`GitVcs::submodule_at`]). None outside a repository or when git can't say.
    #[must_use]
    pub fn listed_files(&self) -> Option<Listing> {
        if !self.in_repository() {
            return None;
        }
        // NUL-separated, so no name is quoted or split; literal pathspecs, though none is given (I7)
        let tracked = run(&[LITERAL, "ls-files", "-z", "--stage"], &self.root)
            .filter(|o| o.status.success())?;
        let untracked = run(
            &[LITERAL, "ls-files", "-z", "--others", "--exclude-standard"],
            &self.root,
        )
        .filter(|o| o.status.success())?;
        let mut listing = Listing::default();
        for entry in String::from_utf8_lossy(&tracked.stdout).split('\0') {
            // `<mode> <object> <stage>\t<path>`; a conflicted file is listed once per stage
            let Some((meta, path)) = entry.split_once('\t') else {
                continue;
            };
            if meta.starts_with("160000 ") {
                let checked_out =
                    std::fs::symlink_metadata(self.root.join(path).join(".git")).is_ok();
                listing.submodules.push(ListedSubmodule {
                    path: path.to_string(),
                    checked_out,
                });
            } else if listing.files.last().is_none_or(|f| f.path != path) {
                listing.files.push(ListedFile {
                    path: path.to_string(),
                    tracked: true,
                });
            }
        }
        for path in String::from_utf8_lossy(&untracked.stdout).split('\0') {
            // A folder (`dir/`) is a repository of its own that this one doesn't record: not its files
            if path.is_empty() || path.ends_with('/') {
                continue;
            }
            listing.files.push(ListedFile {
                path: path.to_string(),
                tracked: false,
            });
        }
        Some(listing)
    }
}

/// The working copy's files and submodules ([`GitVcs::listed_files`]).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Listing {
    pub files: Vec<ListedFile>,
    pub submodules: Vec<ListedSubmodule>,
}

/// A file git tracks, or has untracked and doesn't ignore: its path from the repository's root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedFile {
    pub path: String,
    pub tracked: bool,
}

/// A submodule the repository records: its folder, and whether it is checked out (`git submodule status` says `-`
/// for one that isn't).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedSubmodule {
    pub path: String,
    pub checked_out: bool,
}
