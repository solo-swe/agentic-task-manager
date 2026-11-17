//! The diffs a code review is made of (docs/code-review-design.md §3.3, §3.6, ATM-564): a commit range's, the working
//! tree's against a commit (uncommitted changes, new files included), and files as they are; and a round's files
//! rebuilt from its base and its diff, so two rounds compare without snapshots. Everything that needs an index uses a
//! temporary one (`GIT_INDEX_FILE`): the user's index and working tree are never touched.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use atm_common::py::strip;
use atm_common::{Error, Result};

use crate::GitVcs;

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
        let relative = self.git_with(&["rev-parse", "--git-path", &name], &[], None)?;
        let path = self.root.join(strip(&relative));
        let removed = Removed(path);
        work(&[("GIT_INDEX_FILE", removed.0.as_path())])
    }

    /// The files of a diff (`args` select it, without the format options), binary ones marked.
    fn changed_files(&self, args: &[&str], env: &[(&str, &Path)]) -> Result<Vec<ChangedFile>> {
        let with = |format: &[&str]| {
            let mut all: Vec<&str> = DIFF.to_vec();
            all.extend_from_slice(format);
            all.extend_from_slice(args);
            self.git_with(&all, env, None)
        };
        let binary: Vec<String> = with(&["--numstat", "--no-renames"])?
            .lines()
            .filter_map(|line| line.strip_prefix("-\t-\t").map(str::to_string))
            .collect();
        let mut files = Vec::new();
        for line in with(&["-M", "--name-status"])?.lines() {
            let mut parts = line.split('\t');
            let status = parts.next().and_then(|s| s.chars().next()).unwrap_or('M');
            let (from, path) = match (parts.next(), parts.next()) {
                (Some(from), Some(to)) => (Some(from.to_string()), to.to_string()),
                (Some(path), None) => (None, path.to_string()),
                _ => continue,
            };
            files.push(ChangedFile {
                binary: binary.contains(&path),
                path,
                status,
                from,
            });
        }
        Ok(files)
    }

    fn patch(&self, args: &[&str], env: &[(&str, &Path)]) -> Result<String> {
        let mut all: Vec<&str> = DIFF.to_vec();
        all.push("-M");
        all.extend_from_slice(args);
        self.git_with(&all, env, None)
    }

    /// The diff of a commit range, `base` to `head`, the folders `leave_out` left out (atm's own task folder).
    ///
    /// # Errors
    /// Either isn't a commit, or git fails.
    pub fn diff_range(&self, base: &str, head: &str, leave_out: &[&str]) -> Result<Diff> {
        let spec = pathspec(leave_out);
        let mut args = vec![base, head];
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
    pub fn diff_working(&self, base: &str, leave_out: &[&str]) -> Result<Diff> {
        let spec = pathspec(leave_out);
        self.with_temporary_index(|env| {
            self.git_with(&["read-tree", base], env, None)?;
            self.git_with(&["add", "-A"], env, None)?;
            let mut args = vec!["--cached", base];
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
        self.with_temporary_index(|env| {
            self.git_with(&["read-tree", "--empty"], env, None)?;
            let mut add = vec!["--literal-pathspecs", "add", "-f", "--"];
            add.extend(paths.iter().map(String::as_str));
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
    pub fn tree_after(&self, base: Option<&str>, patch: &str, skip: &[String]) -> Result<String> {
        self.with_temporary_index(|env| {
            match base {
                Some(base) => self.git_with(&["read-tree", base], env, None)?,
                None => self.git_with(&["read-tree", "--empty"], env, None)?,
            };
            if !patch.trim().is_empty() {
                let excludes: Vec<String> = skip.iter().map(|p| format!("--exclude={p}")).collect();
                let mut apply = vec!["apply", "--cached", "--whitespace=nowarn"];
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
    pub fn diff_trees(&self, from: &str, to: &str, paths: &[String]) -> Result<String> {
        let mut args = vec![from, to, "--"];
        args.extend(paths.iter().map(String::as_str));
        let mut all: Vec<&str> = vec!["--literal-pathspecs"];
        all.extend(DIFF);
        all.push("-M");
        all.extend(args);
        self.git_with(&all, &[], None)
    }

    /// A file's text in a tree (or commit), none when it isn't there.
    #[must_use]
    pub fn file_in(&self, tree: &str, path: &str) -> Option<String> {
        self.git_with(&["show", &format!("{tree}:{path}")], &[], None)
            .ok()
    }

    /// A commit's tree.
    ///
    /// # Errors
    /// Not a commit.
    pub fn tree_of(&self, commit: &str) -> Result<String> {
        Ok(
            strip(&self.git_with(&["rev-parse", &format!("{commit}^{{tree}}")], &[], None)?)
                .to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::ChangedFile;
    use crate::GitVcs;

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
        let dir = std::fs::canonicalize(&dir).unwrap();
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
    fn commit(vcs: &GitVcs, message: &str) -> String {
        git(vcs, &["add", "-A"]);
        git(vcs, &["commit", "-q", "-m", message]);
        git(vcs, &["rev-parse", "HEAD"]).trim().to_string()
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

    #[test]
    fn a_rounds_files_are_rebuilt_from_its_base_and_patch_and_two_rounds_compare() {
        let (_repo, vcs) = repo();
        write(&vcs, "a.txt", "one\ntwo\nthree\n");
        let base = commit(&vcs, "base");
        write(&vcs, "a.txt", "one\nTWO\nthree\n");
        let round1 = vcs.diff_working(&base, &[]).unwrap();
        write(&vcs, "a.txt", "one\nTWO\nthree\nfour\n");
        let round2 = vcs.diff_working(&base, &[]).unwrap();

        let tree1 = vcs.tree_after(Some(&base), &round1.patch, &[]).unwrap();
        let tree2 = vcs.tree_after(Some(&base), &round2.patch, &[]).unwrap();
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
