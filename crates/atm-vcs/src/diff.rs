//! The diffs a code review is made of (docs/code-review-design.md §3.3, §3.6, ATM-564): a commit range's, the working
//! tree's against a commit (uncommitted changes, new files included), and files as they are; and a round's files
//! rebuilt from its base and its diff, so two rounds compare without snapshots. Everything that needs an index uses a
//! temporary one (`GIT_INDEX_FILE`): the user's index and working tree are never touched.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use atm_common::py::strip;
use atm_common::{Error, Result};

use crate::{END_OF_OPTIONS, GitVcs, Rev};

/// `path` without Windows' extended prefix (`\\?\C:\…` is `C:\…`, `\\?\UNC\host\…` is `\\host\…`): git can't make a
/// file under a path in that form ("Invalid argument"), which canonicalize gives. Any other path as it is.
fn plain(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(share) = text.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{share}"))
    } else if let Some(local) = text.strip_prefix(r"\\?\") {
        PathBuf::from(local)
    } else {
        path.to_path_buf()
    }
}

/// A file a diff changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    /// Its path, from the working copy's root (`/`-separated).
    pub path: String,
    /// `A` added, `M` modified, `D` deleted, `R` renamed.
    pub status: char,
    /// Where a renamed file was.
    pub from: Option<String>,
    /// A binary file: in the list, never in the patch.
    pub binary: bool,
}

/// A diff: git's patch (text files, three lines of context) and its files.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Diff {
    /// The patch, as `git diff` prints it.
    pub patch: String,
    /// Its files, in git's order.
    pub files: Vec<ChangedFile>,
}

/// A pathspec of the whole working copy but `leave_out`'s folders; none when nothing is left out.
fn pathspec(leave_out: &[&str]) -> Vec<String> {
    if leave_out.is_empty() {
        return Vec::new();
    }
    let mut spec = vec!["--".to_string(), ".".to_string()];
    spec.extend(leave_out.iter().map(|p| format!(":(exclude){p}")));
    spec
}

/// The common options of every diff atm reads: renames found, no colour or external diff tool, paths unquoted.
const DIFF: [&str; 5] = [
    "-c",
    "core.quotepath=off",
    "diff",
    "--no-color",
    "--no-ext-diff",
];

impl GitVcs {
    /// Run git with `env` set and `stdin` fed; its output (unstripped), or git's message as the error.
    fn git_with(
        &self,
        args: &[&str],
        env: &[(&str, &Path)],
        stdin: Option<&[u8]>,
    ) -> Result<String> {
        let mut command = Command::new("git");
        command
            .args(args)
            .current_dir(&self.root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(if stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            });
        for (name, value) in env {
            command.env(name, value);
        }
        let mut child = command
            .spawn()
            .map_err(|e| Error::failed(format!("git isn't available: {e}")))?;
        if let (Some(input), Some(mut pipe)) = (stdin, child.stdin.take()) {
            pipe.write_all(input)
                .map_err(|e| Error::failed(format!("git {}: {e}", args.join(" "))))?;
        }
        let output = child
            .wait_with_output()
            .map_err(|e| Error::failed(format!("git {}: {e}", args.join(" "))))?;
        if !output.status.success() {
            let command = args
                .iter()
                .find(|a| !a.starts_with('-') && **a != "core.quotepath=off");
            return Err(Error::failed(format!(
                "git {} failed: {}",
                command.copied().unwrap_or_default(),
                strip(&String::from_utf8_lossy(&output.stderr))
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// A revision as the commit's SHA: what reaches git as a diff's end (a [`Rev`] is never a name git could read as
    /// an option, whoever wrote it: the browser's write routes, ATM-572; a round's files, ATM-659).
    ///
    /// # Errors
    /// `NotFound` for one that names no commit.
    fn commit_sha(&self, revision: &Rev) -> Result<String> {
        self.resolve(revision)
            .ok_or_else(|| Error::NotFound(format!("{revision}: no such commit")))
    }

    /// A temporary index in the git folder, removed when `work` is done; `work` gets the environment that uses it.
    fn with_temporary_index<T>(
        &self,
        work: impl FnOnce(&[(&str, &Path)]) -> Result<T>,
    ) -> Result<T> {
        struct Removed(PathBuf);
        impl Drop for Removed {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let name = format!("atm-review-index-{}-{nanos}", std::process::id());
        let answer = self.git_with(&["rev-parse", "--git-path", &name], &[], None)?;
        let answer = strip(&answer);
        // git answers with `/` between the parts: joined part by part, as a root in Windows' extended form
        // (`\\?\C:\…`, which canonicalize gives) takes no `/`; an absolute answer (a worktree's) as it is
        let path = if Path::new(answer).is_absolute() {
            PathBuf::from(answer)
        } else {
            let mut path = plain(&self.root);
            path.extend(answer.split('/').filter(|part| !part.is_empty()));
            path
        };
        let removed = Removed(path);
        work(&[("GIT_INDEX_FILE", removed.0.as_path())])
    }

    /// The files of a diff (`args` select it, without the format options), binary ones marked; their paths as the
    /// project has them (a submodule's under its folder).
    fn changed_files(&self, args: &[&str], env: &[(&str, &Path)]) -> Result<Vec<ChangedFile>> {
        let with = |format: &[&str]| {
            let mut all: Vec<&str> = DIFF.to_vec();
            all.extend_from_slice(format);
            all.extend_from_slice(args);
            self.git_with(&all, env, None)
        };
        // NUL-separated (`-z`, ATM-670, audit R14): a path is as it is, never C-quoted for a tab or a quote in it
        let binary: Vec<String> = with(&["--numstat", "--no-renames", "-z"])?
            .split('\0')
            .filter_map(|record| record.strip_prefix("-\t-\t").map(str::to_string))
            .collect();
        let mut files = Vec::new();
        let listed = with(&["-M", "--name-status", "-z"])?;
        let mut fields = listed.split('\0').filter(|f| !f.is_empty());
        // Each change is its status, then its path; a rename or a copy, its old path and its new one
        while let Some(code) = fields.next() {
            let status = code.chars().next().unwrap_or('M');
            let (from, path) = if matches!(status, 'R' | 'C') {
                match (fields.next(), fields.next()) {
                    (Some(from), Some(to)) => (Some(from.to_string()), to.to_string()),
                    _ => break,
                }
            } else {
                match fields.next() {
                    Some(path) => (None, path.to_string()),
                    None => break,
                }
            };
            files.push(ChangedFile {
                binary: binary.contains(&path),
                path: self.shown_path(&path),
                status,
                from: from.map(|f| self.shown_path(&f)),
            });
        }
        Ok(files)
    }

    /// A submodule's diffs name its files as the project has them: under its folder (`a/lib/b.txt`).
    fn prefixes(&self) -> Vec<String> {
        match self.shown_folder() {
            Some(folder) => vec![
                format!("--src-prefix=a/{folder}/"),
                format!("--dst-prefix=b/{folder}/"),
            ],
            None => Vec::new(),
        }
    }

    fn patch(&self, args: &[&str], env: &[(&str, &Path)]) -> Result<String> {
        let prefixes = self.prefixes();
        let mut all: Vec<&str> = DIFF.to_vec();
        all.push("-M");
        all.extend(prefixes.iter().map(String::as_str));
        all.extend_from_slice(args);
        self.git_with(&all, env, None)
    }

    /// A path as this repository has it, or `Invalid` for one outside it (a submodule's round names only its files).
    fn inside<'a>(&self, path: &'a str) -> Result<&'a str> {
        self.inside_path(path).ok_or_else(|| {
            Error::Invalid(format!(
                "{path} isn't in the submodule {}",
                self.shown_folder().unwrap_or_default()
            ))
        })
    }

    /// The diff from one tree (or commit) to another, with the files it changes (ATM-725: a review's views, each from
    /// one round's result, or the review's base, to another's): only `only` when any are named, else everything but
    /// the folders `leave_out` (atm's own task folder).
    ///
    /// # Errors
    /// Either isn't a tree or a commit, a path outside a submodule's folder, or git fails.
    pub fn diff_between(
        &self,
        from: &Rev,
        to: &Rev,
        only: &[String],
        leave_out: &[&str],
    ) -> Result<Diff> {
        let spec = if only.is_empty() {
            pathspec(leave_out)
        } else {
            let mut spec = vec!["--".to_string()];
            for path in only {
                spec.push(format!(":(literal){}", self.inside(path)?));
            }
            spec
        };
        let mut args = vec![END_OF_OPTIONS, from.as_str(), to.as_str()];
        args.extend(spec.iter().map(String::as_str));
        Ok(Diff {
            patch: self.patch(&args, &[])?,
            files: self.changed_files(&args, &[])?,
        })
    }

    /// The diff of a commit range, `base` to `head`, the folders `leave_out` left out (atm's own task folder).
    ///
    /// # Errors
    /// Either isn't a commit, or git fails.
    pub fn diff_range(&self, base: &Rev, head: &Rev, leave_out: &[&str]) -> Result<Diff> {
        let spec = pathspec(leave_out);
        let (base, head) = (self.commit_sha(base)?, self.commit_sha(head)?);
        let mut args = vec![END_OF_OPTIONS, base.as_str(), head.as_str()];
        args.extend(spec.iter().map(String::as_str));
        Ok(Diff {
            patch: self.patch(&args, &[])?,
            files: self.changed_files(&args, &[])?,
        })
    }

    /// The working tree's changes against `base`, new files included (ignored ones not), the folders `leave_out` left
    /// out, as if everything were staged: through a temporary index, so the user's index is untouched.
    ///
    /// # Errors
    /// `base` isn't a commit, or git fails.
    pub fn diff_working(&self, base: &Rev, leave_out: &[&str]) -> Result<Diff> {
        let spec = pathspec(leave_out);
        let base = self.commit_sha(base)?;
        let base = base.as_str();
        self.with_temporary_index(|env| {
            self.git_with(&["read-tree", END_OF_OPTIONS, base], env, None)?;
            self.git_with(&["add", "-A"], env, None)?;
            let mut args = vec!["--cached", END_OF_OPTIONS, base];
            args.extend(spec.iter().map(String::as_str));
            Ok(Diff {
                patch: self.patch(&args, env)?,
                files: self.changed_files(&args, env)?,
            })
        })
    }

    /// Files as they are in the working tree, as if each were added (a plan, a design reviewed whole).
    ///
    /// # Errors
    /// A file git can't add, or git fails.
    pub fn diff_files(&self, paths: &[String]) -> Result<Diff> {
        let empty = self.empty_tree()?;
        let inside: Vec<&str> = paths
            .iter()
            .map(|p| self.inside(p))
            .collect::<Result<_>>()?;
        self.with_temporary_index(|env| {
            self.git_with(&["read-tree", "--empty"], env, None)?;
            let mut add = vec!["--literal-pathspecs", "add", "-f", "--"];
            add.extend(inside.iter().copied());
            self.git_with(&add, env, None)?;
            let args = ["--cached", empty.as_str()];
            Ok(Diff {
                patch: self.patch(&args, env)?,
                files: self.changed_files(&args, env)?,
            })
        })
    }

    /// The empty tree's ID (it depends on the repository's hash).
    ///
    /// # Errors
    /// git fails.
    pub fn empty_tree(&self) -> Result<String> {
        Ok(
            strip(&self.git_with(&["hash-object", "-t", "tree", "--stdin"], &[], Some(b""))?)
                .to_string(),
        )
    }

    /// The tree a patch makes of `base` (a commit or a tree; none: the empty tree), its files `skip` left out (the
    /// binary ones, which a patch doesn't carry): a round's files, rebuilt. Written to git's object store only.
    ///
    /// # Errors
    /// The patch doesn't apply to `base`, or git fails.
    pub fn tree_after(&self, base: Option<&Rev>, patch: &str, skip: &[String]) -> Result<String> {
        self.with_temporary_index(|env| {
            match base {
                Some(base) => {
                    self.git_with(&["read-tree", END_OF_OPTIONS, base.as_str()], env, None)?
                }
                None => self.git_with(&["read-tree", "--empty"], env, None)?,
            };
            if !patch.trim().is_empty() {
                // A submodule's patch names its files under its folder: applied without it (`-p` one more per part)
                let depth = format!(
                    "-p{}",
                    1 + self.shown_folder().map_or(0, |f| f.split('/').count())
                );
                let excludes: Vec<String> = skip
                    .iter()
                    .map(|p| format!("--exclude={}", self.inside_path(p).unwrap_or(p)))
                    .collect();
                let mut apply = vec!["apply", "--cached", "--whitespace=nowarn", depth.as_str()];
                apply.extend(excludes.iter().map(String::as_str));
                apply.push("-");
                self.git_with(&apply, env, Some(patch.as_bytes()))?;
            }
            Ok(strip(&self.git_with(&["write-tree"], env, None)?).to_string())
        })
    }

    /// The diff between two trees (or commits), only `paths` when any are named.
    ///
    /// # Errors
    /// Either isn't one, or git fails.
    pub fn diff_trees(&self, from: &Rev, to: &Rev, paths: &[String]) -> Result<String> {
        let inside: Vec<&str> = paths
            .iter()
            .map(|p| self.inside(p))
            .collect::<Result<_>>()?;
        let prefixes = self.prefixes();
        let mut all: Vec<&str> = vec!["--literal-pathspecs"];
        all.extend(DIFF);
        all.push("-M");
        all.extend(prefixes.iter().map(String::as_str));
        all.extend([END_OF_OPTIONS, from.as_str(), to.as_str(), "--"]);
        all.extend(inside);
        self.git_with(&all, &[], None)
    }

    /// A file's text in a tree (or commit), none when it isn't there (or, in a submodule's, isn't under its folder).
    #[must_use]
    pub fn file_in(&self, tree: &Rev, path: &str) -> Option<String> {
        let path = self.inside_path(path)?;
        self.git_with(
            &["show", END_OF_OPTIONS, &format!("{tree}:{path}")],
            &[],
            None,
        )
        .ok()
    }

    /// A commit's tree.
    ///
    /// # Errors
    /// Not a commit.
    pub fn tree_of(&self, commit: &Rev) -> Result<String> {
        // `--verify`: without it, rev-parse echoes `--end-of-options` back as one of its answers
        Ok(strip(&self.git_with(
            &[
                "rev-parse",
                "--verify",
                END_OF_OPTIONS,
                &format!("{commit}^{{tree}}"),
            ],
            &[],
            None,
        )?)
        .to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::ChangedFile;
    use crate::{GitVcs, Rev};

    /// A revision, by a name the test gives.
    fn rev(name: &str) -> Rev {
        Rev::new(name).unwrap()
    }

    struct Repo(std::path::PathBuf);
    impl Drop for Repo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn repo() -> (Repo, GitVcs) {
        // A count too: tests run at once, and two can start in the same nanosecond
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let count = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "atm-vcs-diff-{}-{count}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        // Without Windows' `\\?\` prefix, as atm and git name the folder (a submodule's URL too)
        let dir = atm_common::pathtext::canonical(&dir).unwrap();
        let vcs = GitVcs::new(&dir);
        git(&vcs, &["init", "-q"]);
        (Repo(dir), vcs)
    }
    fn git(vcs: &GitVcs, args: &[&str]) -> String {
        let mut all = vec!["-c", "user.name=t", "-c", "user.email=t@example.com"];
        all.extend_from_slice(args);
        vcs.git_with(&all, &[], None).unwrap()
    }
    fn write(vcs: &GitVcs, path: &str, text: &str) {
        std::fs::write(vcs.root.join(path), text).unwrap();
    }
    fn commit(vcs: &GitVcs, message: &str) -> Rev {
        git(vcs, &["add", "-A"]);
        git(vcs, &["commit", "-q", "-m", message]);
        head(vcs)
    }
    fn head(vcs: &GitVcs) -> Rev {
        Rev::object_id(git(vcs, &["rev-parse", "HEAD"]).trim()).unwrap()
    }

    #[test]
    fn a_ranges_diff_has_its_text_files_and_lists_a_binary_one() {
        let (_repo, vcs) = repo();
        write(&vcs, "a.txt", "one\n");
        let base = commit(&vcs, "base");
        write(&vcs, "a.txt", "one\ntwo\n");
        std::fs::write(vcs.root.join("logo.png"), [0u8, 159, 146, 150, 0, 1]).unwrap();
        let head = commit(&vcs, "head");
        let diff = vcs.diff_range(&base, &head, &[]).unwrap();
        assert!(diff.patch.contains("+two"), "{}", diff.patch);
        assert!(
            diff.patch.contains("Binary files"),
            "only named, never carried"
        );
        assert_eq!(
            diff.files,
            [
                ChangedFile {
                    path: "a.txt".into(),
                    status: 'M',
                    from: None,
                    binary: false
                },
                ChangedFile {
                    path: "logo.png".into(),
                    status: 'A',
                    from: None,
                    binary: true
                },
            ]
        );
    }

    #[test]
    fn the_working_trees_diff_has_new_files_and_leaves_the_users_index_alone() {
        let (_repo, vcs) = repo();
        write(&vcs, "a.txt", "one\n");
        write(&vcs, ".gitignore", "secret\n");
        let base = commit(&vcs, "base");
        write(&vcs, "a.txt", "one\nstaged\n");
        git(&vcs, &["add", "a.txt"]);
        write(&vcs, "a.txt", "one\nstaged\nnot staged\n");
        write(&vcs, "new.txt", "brand new\n");
        write(&vcs, "secret", "never\n");
        let staged_before = git(&vcs, &["diff", "--cached"]);
        let diff = vcs.diff_working(&base, &[]).unwrap();
        assert!(
            diff.patch.contains("+not staged") && diff.patch.contains("+brand new"),
            "{}",
            diff.patch
        );
        assert!(!diff.patch.contains("never"), "ignored files stay out");
        assert_eq!(
            git(&vcs, &["diff", "--cached"]),
            staged_before,
            "the user's index is untouched"
        );
        let status = git(&vcs, &["status", "--porcelain"]);
        assert!(status.contains("?? new.txt"), "{status}");
    }

    #[test]
    fn a_folder_left_out_is_in_neither_diff() {
        let (_repo, vcs) = repo();
        std::fs::create_dir(vcs.root.join(".agentic-task-manager")).unwrap();
        write(&vcs, ".agentic-task-manager/tasks.json", "{}\n");
        write(&vcs, "a.txt", "one\n");
        let base = commit(&vcs, "base");
        write(
            &vcs,
            ".agentic-task-manager/tasks.json",
            "{\"changed\": true}\n",
        );
        write(&vcs, "a.txt", "one\ntwo\n");
        let working = vcs.diff_working(&base, &[".agentic-task-manager"]).unwrap();
        assert_eq!(
            working
                .files
                .iter()
                .map(|f| f.path.as_str())
                .collect::<Vec<_>>(),
            ["a.txt"]
        );
        assert!(!working.patch.contains("changed"));
        let head = commit(&vcs, "head");
        let range = vcs
            .diff_range(&base, &head, &[".agentic-task-manager"])
            .unwrap();
        assert_eq!(range.files.len(), 1);
        assert!(!range.patch.contains("tasks.json"));
    }

    #[test]
    fn git_gets_paths_without_the_extended_prefix_and_revisions_never_as_options() {
        use atm_common::Error;
        use std::path::Path;
        assert_eq!(
            super::plain(Path::new(r"\\?\C:\Temp\repo")),
            Path::new(r"C:\Temp\repo")
        );
        assert_eq!(
            super::plain(Path::new(r"\\?\UNC\host\share\repo")),
            Path::new(r"\\host\share\repo")
        );
        assert_eq!(super::plain(Path::new("/tmp/repo")), Path::new("/tmp/repo"));
        let (_repo, vcs) = repo();
        write(&vcs, "a.txt", "a\n");
        git(&vcs, &["add", "-A"]);
        git(&vcs, &["commit", "-q", "-m", "a"]);
        // A revision that would be an option to git is no Rev, so none reaches a diff (ATM-659)
        for bad in ["--output=x", "-p", ""] {
            assert!(matches!(Rev::new(bad), Err(Error::Invalid(_))), "{bad}");
        }
        assert!(!vcs.root.join("x").exists());
        assert!(matches!(
            vcs.diff_range(&rev("no-such"), &Rev::head(), &[]),
            Err(Error::NotFound(_))
        ));
    }

    /// ATM-670 (audit R14): without -z, git C-quoted a path with a tab or a quote ("a\tb.md"), and that was the name
    #[cfg(unix)]
    #[test]
    fn a_path_with_a_tab_a_quote_or_a_rename_is_listed_as_it_is() {
        let (_repo, vcs) = repo();
        write(&vcs, "plain.md", "one\ntwo\nthree\nfour\n");
        let base = commit(&vcs, "base");
        write(&vcs, "a\tb \"c\".md", "new\n");
        git(&vcs, &["mv", "plain.md", "éé moved.md"]);
        let head = commit(&vcs, "head");
        let diff = vcs.diff_range(&base, &head, &[]).unwrap();
        let mut files: Vec<(String, char, Option<String>)> = diff
            .files
            .into_iter()
            .map(|f| (f.path, f.status, f.from))
            .collect();
        files.sort();
        assert_eq!(
            files,
            [
                ("a\tb \"c\".md".to_string(), 'A', None),
                ("éé moved.md".to_string(), 'R', Some("plain.md".to_string())),
            ]
        );
    }

    #[test]
    fn files_as_they_are_are_a_diff_from_nothing() {
        let (_repo, vcs) = repo();
        write(&vcs, "plan.md", "# Plan\n\nDo it.\n");
        commit(&vcs, "plan");
        let diff = vcs.diff_files(&["plan.md".into()]).unwrap();
        assert!(
            diff.patch.contains("new file mode") && diff.patch.contains("+# Plan"),
            "{}",
            diff.patch
        );
        assert_eq!(diff.files[0].status, 'A');
    }

    /// A repository with the submodule `lib` (its own repository, one commit), committed: (the repo, its vcs, the
    /// submodule's repository, kept alive as long as the test, its vcs in the checkout).
    fn with_submodule() -> (Repo, GitVcs, Repo, GitVcs) {
        let (sub_repo, sub) = repo();
        write(&sub, "b.txt", "one\n");
        commit(&sub, "sub base");
        let (repo, vcs) = repo();
        write(&vcs, "a.txt", "one\n");
        let source = sub.root.to_string_lossy().into_owned();
        git(
            &vcs,
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                "-q",
                &source,
                "lib",
            ],
        );
        commit(&vcs, "base");
        let checkout = GitVcs::new(&vcs.root.join("lib"));
        (repo, vcs, sub_repo, checkout)
    }

    #[test]
    fn a_range_that_moves_a_submodule_lists_it_and_rebuilds_its_tree() {
        let (_repo, vcs, _sub, lib) = with_submodule();
        let base = head(&vcs);
        write(&lib, "b.txt", "one\ntwo\n");
        commit(&lib, "sub change");
        write(&vcs, "a.txt", "one\nchanged\n");
        let head = commit(&vcs, "move lib");
        let diff = vcs.diff_range(&base, &head, &[]).unwrap();
        assert!(diff.patch.contains("+Subproject commit "), "{}", diff.patch);
        assert!(
            diff.files
                .iter()
                .any(|f| f.path == "lib" && f.status == 'M')
        );
        let tree = vcs.tree_after(Some(&base), &diff.patch, &[]).unwrap();
        assert_eq!(
            tree,
            vcs.tree_of(&head).unwrap(),
            "the round's tree is the head's"
        );
    }

    #[test]
    fn uncommitted_changes_with_a_moved_and_dirty_submodule_rebuild_their_tree() {
        let (_repo, vcs, _sub, lib) = with_submodule();
        let base = head(&vcs);
        write(&lib, "b.txt", "one\ntwo\n");
        commit(&lib, "sub change");
        write(&lib, "b.txt", "one\ntwo\nthree, not committed\n");
        write(&vcs, "a.txt", "one\nchanged\n");
        let diff = vcs.diff_working(&base, &[]).unwrap();
        let tree = vcs.tree_after(Some(&base), &diff.patch, &[]);
        assert!(tree.is_ok(), "{tree:?}\n{}", diff.patch);
    }

    #[test]
    fn a_submodules_repository_diffs_its_commits_with_the_projects_paths() {
        let (_repo, vcs, _sub, lib) = with_submodule();
        let base = head(&lib);
        write(&lib, "b.txt", "one\ntwo\n");
        write(&lib, "c.txt", "new\n");
        let head = commit(&lib, "sub change");
        let sub = vcs.submodule_at("lib").expect("lib is a submodule");
        assert_eq!(sub.shown_folder(), Some("lib"));
        let diff = sub.diff_range(&base, &head, &[]).unwrap();
        assert!(
            diff.patch.contains("diff --git a/lib/b.txt b/lib/b.txt")
                && diff.patch.contains("+++ b/lib/c.txt"),
            "{}",
            diff.patch
        );
        assert_eq!(
            diff.files
                .iter()
                .map(|f| f.path.as_str())
                .collect::<Vec<_>>(),
            ["lib/b.txt", "lib/c.txt"]
        );
        // Its round's files rebuild in its own repository, read and compared by the project's paths
        let tree = rev(&sub.tree_after(Some(&base), &diff.patch, &[]).unwrap());
        assert_eq!(tree.as_str(), sub.tree_of(&head).unwrap());
        assert_eq!(sub.file_in(&tree, "lib/b.txt").unwrap(), "one\ntwo\n");
        assert_eq!(
            sub.file_in(&tree, "b.txt"),
            None,
            "a path outside it is none"
        );
        let between = sub.diff_trees(&base, &tree, &["lib/b.txt".into()]).unwrap();
        assert!(
            between.contains("+++ b/lib/b.txt") && between.contains("+two"),
            "{between}"
        );
        assert!(matches!(
            sub.diff_trees(&base, &tree, &["a.txt".into()]),
            Err(atm_common::Error::Invalid(_))
        ));
        // Uncommitted changes in it, and its files as they are
        write(&lib, "b.txt", "one\ntwo\nthree\n");
        let working = sub.diff_working(&head, &[]).unwrap();
        assert_eq!(working.files[0].path, "lib/b.txt");
        let whole = sub.diff_files(&["lib/b.txt".into()]).unwrap();
        assert!(whole.patch.contains("+++ b/lib/b.txt"), "{}", whole.patch);
        assert!(sub.tree_after(None, &whole.patch, &[]).is_ok());
    }

    #[test]
    fn only_a_checked_out_submodules_folder_opens_its_repository() {
        let (_repo, vcs, _sub, _lib) = with_submodule();
        assert!(
            vcs.submodule_at("lib/").is_some(),
            "a trailing slash is the same folder"
        );
        for not_one in ["", ".", "a.txt", "nothing", "lib/inner"] {
            assert!(vcs.submodule_at(not_one).is_none(), "{not_one}");
        }
        assert_eq!(
            vcs.submodule_containing(&vcs.root.join("lib")).as_deref(),
            Some("lib")
        );
        std::fs::create_dir(vcs.root.join("lib/deeper")).unwrap();
        assert_eq!(
            vcs.submodule_containing(&vcs.root.join("lib/deeper"))
                .as_deref(),
            Some("lib")
        );
        assert_eq!(vcs.submodule_containing(&vcs.root), None);
        assert_eq!(vcs.submodule_containing(&std::env::temp_dir()), None);
        assert_eq!(vcs.shown_folder(), None);
    }

    #[test]
    fn a_rounds_files_are_rebuilt_from_its_base_and_patch_and_two_rounds_compare() {
        let (_repo, vcs) = repo();
        write(&vcs, "a.txt", "one\ntwo\nthree\n");
        let base = commit(&vcs, "base");
        write(&vcs, "a.txt", "one\nTWO\nthree\n");
        let round1 = vcs.diff_working(&base, &[]).unwrap();
        write(&vcs, "a.txt", "one\nTWO\nthree\nfour\n");
        let round2 = vcs.diff_working(&base, &[]).unwrap();

        let tree1 = rev(&vcs.tree_after(Some(&base), &round1.patch, &[]).unwrap());
        let tree2 = rev(&vcs.tree_after(Some(&base), &round2.patch, &[]).unwrap());
        assert_eq!(vcs.file_in(&tree1, "a.txt").unwrap(), "one\nTWO\nthree\n");
        let between = vcs.diff_trees(&tree1, &tree2, &["a.txt".into()]).unwrap();
        assert!(
            between.contains("+four") && !between.contains("+TWO"),
            "{between}"
        );
        // Files reviewed whole rebuild from nothing
        let empty = vcs.tree_after(None, "", &[]).unwrap();
        assert_eq!(empty, vcs.empty_tree().unwrap());
    }
}
