//! Setting the task folder up in git (the Python atm's `db/jsongit/merge.py` `ensure_merge_driver` and
//! `ensure_archive_merge`): the merge driver registered in the clone's config, and `.gitattributes` naming the task
//! files it merges.

use std::path::Path;

use atm_common::Shown;

use atm_common::{Error, Result};

use super::paths::Paths;
use crate::git::git;

/// What `.gitattributes` gets: `atm` itself is defined in each clone's git config, which can't be committed.
const GITATTRIBUTES_LINES: &str = "# 'atm' is defined in each clone's git config (merge.atm.driver), registered by the atm plugin on every run.\n\
     # Without it, git merges tasks.json line by line as usual.\n\
     tasks.json merge=atm\n";
/// The archive's line, added when a project first archives.
const ARCHIVE_ATTRIBUTES_LINE: &str = "tasks-archive.json merge=atm";

/// Whether `folder` is in a git repository: the one way atm asks.
#[must_use]
pub fn in_git(folder: &Path) -> bool {
    git(&["rev-parse", "--git-dir"], folder).is_some()
}

/// `path` relative to `root`, as git is given it (the path itself when it isn't under `root`), with `/` between its
/// parts on every platform: git takes them on Windows too, and atm's output is the same everywhere.
#[must_use]
pub fn rel(root: &Path, path: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(relative) => relative
            .components()
            .map(|part| part.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"),
        Err(_) => path.shown(),
    }
}

/// Whether git tracks `path` in the repository at `root`.
#[must_use]
pub fn is_tracked(root: &Path, path: &Path) -> bool {
    git(
        &["ls-files", "--error-unmatch", "--", &rel(root, path)],
        root,
    )
    .is_some()
}

/// Stage `paths` in the repository at `root` (when it is one).
pub fn stage(root: &Path, paths: &[&Path]) {
    let relative: Vec<String> = paths.iter().map(|p| rel(root, p)).collect();
    let mut args = vec!["add", "--"];
    args.extend(relative.iter().map(String::as_str));
    // Staging is a courtesy: the change is in the file either way
    let _ = git(&args, root);
}

/// The command git runs to merge the task files: this atm, quoted for the shell.
#[must_use]
pub fn launcher_command() -> String {
    let exe = std::env::current_exe().map_or_else(|_| "atm".into(), |p| p.shown());
    format!("'{}' db merge-driver %O %A %B", exe.replace('\'', "'\\''"))
}

fn write(path: &Path, text: &str) -> Result<()> {
    std::fs::write(path, text).map_err(|e| Error::failed(format!("{}: {e}", path.shown())))
}

/// In git: `tasks.json merge=atm` in the task folder's `.gitattributes` (staged when added), and the driver registered
/// in the clone's config, pointing at this atm. Returns whether `.gitattributes` changed.
///
/// # Errors
/// When `.gitattributes` can't be written.
pub fn ensure_merge_driver(paths: &Paths) -> Result<bool> {
    let root = &paths.root;
    if !in_git(root) {
        return Ok(false);
    }
    let driver = launcher_command();
    for (key, value) in [
        ("merge.atm.name", "atm: merge tasks.json task by task"),
        ("merge.atm.driver", driver.as_str()),
    ] {
        if git(&["config", "--get", key], root).as_deref() != Some(value) {
            let _ = git(&["config", key, value], root);
        }
    }
    let attributes = paths.dir.join(".gitattributes");
    let text = std::fs::read_to_string(&attributes).unwrap_or_default();
    if text.lines().any(|l| l == "tasks.json merge=atm") {
        return Ok(false);
    }
    std::fs::create_dir_all(&paths.dir)
        .map_err(|e| Error::failed(format!("{}: {e}", paths.dir.shown())))?;
    let separator = if text.is_empty() || text.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    write(
        &attributes,
        &format!("{text}{separator}{GITATTRIBUTES_LINES}"),
    )?;
    stage(root, &[&attributes]);
    Ok(true)
}

/// In git: the archive merges task by task too. Added when a project first archives, so projects that never do see no
/// change. Returns whether `.gitattributes` changed (staged).
///
/// # Errors
/// When `.gitattributes` can't be read or written.
pub fn ensure_archive_merge(paths: &Paths) -> Result<bool> {
    let root = &paths.root;
    if !in_git(root) {
        return Ok(false);
    }
    ensure_merge_driver(paths)?;
    let attributes = paths.dir.join(".gitattributes");
    let text = std::fs::read_to_string(&attributes)
        .map_err(|e| Error::failed(format!("{}: {e}", attributes.shown())))?;
    if text.lines().any(|l| l == ARCHIVE_ATTRIBUTES_LINE) {
        return Ok(false);
    }
    let separator = if text.ends_with('\n') { "" } else { "\n" };
    write(
        &attributes,
        &format!("{text}{separator}{ARCHIVE_ATTRIBUTES_LINE}\n"),
    )?;
    stage(root, &[&attributes]);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_launcher_is_quoted_for_the_shell() {
        let command = launcher_command();
        assert!(command.starts_with('\'') && command.ends_with("' db merge-driver %O %A %B"));
        assert_eq!(rel(Path::new("/p"), Path::new("/p/a/b")), "a/b");
        assert_eq!(rel(Path::new("/p"), Path::new("/q")), "/q");
    }

    #[test]
    fn outside_git_there_is_nothing_to_set_up() {
        let dir = crate::test_dir::TestDir::new();
        let paths = super::super::paths::find_paths(dir.path()).unwrap();
        if !in_git(dir.path()) {
            assert!(!ensure_merge_driver(&paths).unwrap());
            assert!(!ensure_archive_merge(&paths).unwrap());
        }
    }
}
