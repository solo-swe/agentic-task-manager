//! Where a project's task files are (the Python atm's `db/jsongit/paths.py`): the project root (git's top level, or
//! the nearest folder with a task folder), the task folder, and the checks that keep atm inside the project.

use std::fs;
use std::path::{Path, PathBuf};

use atm_common::{ARCHIVE_FILE, Error, Result, TASK_DIR, TASKS_FILE};

use crate::git::git;

/// The files atm reads and writes in the task folder; a repository can commit any of them as a symlink.
const FOLDER_FILES: [&str; 4] = [TASKS_FILE, ARCHIVE_FILE, ".gitignore", ".gitattributes"];

/// Where a project's task files are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    /// The project's root: git's top level, else the folder with the task folder.
    pub root: PathBuf,
    /// The task folder, `<root>/.agentic-task-manager`.
    pub dir: PathBuf,
    /// The task file, `<dir>/tasks.json`.
    pub tasks: PathBuf,
}

impl Paths {
    /// The archive, `<dir>/tasks-archive.json`.
    #[must_use]
    pub fn archive(&self) -> PathBuf {
        self.dir.join(ARCHIVE_FILE)
    }
}

fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
}

/// Refuse a task folder atm would read or write outside the project through (docs/security-review.md F7): a cloned
/// repository can commit `.agentic-task-manager` or its files as symlinks to anything the user can write.
///
/// # Errors
/// When the task folder or one of its files is a symlink, or the folder leads outside the project.
pub fn check_task_folder(root: &Path, task_dir: &Path) -> Result<()> {
    if is_symlink(task_dir) {
        return Err(Error::failed(format!(
            "{} is a symlink; atm works only on a real folder inside the project",
            task_dir.display()
        )));
    }
    if !task_dir.is_dir() {
        return Ok(());
    }
    let real_root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let real_dir = fs::canonicalize(task_dir).unwrap_or_else(|_| task_dir.to_path_buf());
    if !real_dir.starts_with(&real_root) {
        return Err(Error::failed(format!(
            "{} leads outside the project ({})",
            task_dir.display(),
            real_dir.display()
        )));
    }
    for name in FOLDER_FILES {
        let file = task_dir.join(name);
        if is_symlink(&file) {
            return Err(Error::failed(format!(
                "{} is a symlink; atm reads and writes only regular files there",
                file.display()
            )));
        }
    }
    Ok(())
}

/// The project's root for `cwd`: git's top level; outside git, the nearest folder with a task folder or
/// `docs/tasks.json`, else `cwd` itself.
#[must_use]
pub fn project_root(cwd: &Path) -> PathBuf {
    if let Some(top) = git(&["rev-parse", "--show-toplevel"], cwd) {
        return PathBuf::from(top);
    }
    let mut d = cwd;
    loop {
        if d.join(TASK_DIR).is_dir() || d.join("docs").join(TASKS_FILE).is_file() {
            return d.to_path_buf();
        }
        match d.parent() {
            Some(parent) => d = parent,
            None => return cwd.to_path_buf(),
        }
    }
}

/// The main worktree's folder for a project root: itself, unless it is a linked git worktree (the registry lists a
/// repository once, by its main worktree).
#[must_use]
pub fn main_worktree(root: &Path) -> PathBuf {
    if let Some(common) = git(&["rev-parse", "--git-common-dir"], root) {
        let normal =
            crate::jsongit::logo::normalize(&atm_common::home::absolute(&root.join(common)));
        if normal.file_name().is_some_and(|n| n == ".git") {
            return normal
                .parent()
                .map_or_else(|| root.to_path_buf(), Path::to_path_buf);
        }
    }
    root.to_path_buf()
}

/// Where task files were kept before `.agentic-task-manager/`, in the order they are looked for.
const LEGACY_DIRS: [&str; 2] = ["docs", "."];
/// The dashboard file an older atm put beside the task file.
pub const DASHBOARD_FILE: &str = "tasks-dashboard.html";

/// A task file left where an older atm kept it, with the dashboard copy beside it if there is one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Legacy {
    /// The old `tasks.json`.
    pub tasks: PathBuf,
    /// The old `tasks-dashboard.html` beside it.
    pub dashboard: Option<PathBuf>,
}

/// The task file left in a legacy location of the project at `root` (`docs/`, then the root itself), if any.
#[must_use]
pub fn legacy_files(root: &Path) -> Option<Legacy> {
    LEGACY_DIRS.iter().find_map(|d| {
        let folder = if *d == "." {
            root.to_path_buf()
        } else {
            root.join(d)
        };
        let tasks = folder.join(TASKS_FILE);
        tasks.is_file().then(|| {
            let dashboard = folder.join(DASHBOARD_FILE);
            Legacy {
                tasks,
                dashboard: dashboard.is_file().then_some(dashboard),
            }
        })
    })
}

/// Where the task files of the project at or above `cwd` are, after the task folder's checks.
///
/// # Errors
/// When the task folder fails [`check_task_folder`].
pub fn find_paths(cwd: &Path) -> Result<Paths> {
    let root = project_root(cwd);
    let dir = root.join(TASK_DIR);
    check_task_folder(&root, &dir)?;
    let tasks = dir.join(TASKS_FILE);
    Ok(Paths { root, dir, tasks })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_dir::TestDir;

    #[test]
    fn outside_git_the_root_is_the_nearest_folder_with_a_task_folder() {
        let project = TestDir::new();
        fs::create_dir_all(project.path().join(TASK_DIR)).unwrap();
        let deep = project.path().join("a/b");
        fs::create_dir_all(&deep).unwrap();
        let paths = find_paths(&deep).unwrap();
        assert_eq!(
            fs::canonicalize(&paths.root).unwrap(),
            fs::canonicalize(project.path()).unwrap()
        );
        assert_eq!(paths.tasks, paths.root.join(TASK_DIR).join(TASKS_FILE));
        assert_eq!(paths.archive(), paths.dir.join(ARCHIVE_FILE));
    }

    #[test]
    fn without_a_task_folder_the_root_is_where_atm_runs() {
        let folder = TestDir::new();
        assert_eq!(project_root(folder.path()), folder.path());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_task_folder_or_file_is_refused() {
        use std::os::unix::fs::symlink;
        let project = TestDir::new();
        let elsewhere = TestDir::new();
        symlink(elsewhere.path(), project.path().join(TASK_DIR)).unwrap();
        let error = check_task_folder(project.path(), &project.path().join(TASK_DIR)).unwrap_err();
        assert!(
            error
                .to_string()
                .ends_with("is a symlink; atm works only on a real folder inside the project")
        );

        let other = TestDir::new();
        let dir = other.path().join(TASK_DIR);
        fs::create_dir(&dir).unwrap();
        symlink(elsewhere.path().join("x"), dir.join(TASKS_FILE)).unwrap();
        let error = check_task_folder(other.path(), &dir).unwrap_err();
        assert!(
            error
                .to_string()
                .ends_with("is a symlink; atm reads and writes only regular files there")
        );
    }
}
