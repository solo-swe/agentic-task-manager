//! Opening a project (and, as later slices come, the registry of projects): the one place that picks the store and the version control, and the actor.
//!
//! Part of atm's Rust port (T-415): docs/porting-to-rust.md, docs/layered-architecture.md, docs/guides/rust.md.

use std::path::Path;

use atm_common::Result;
use atm_db::JsonGitStore;
use atm_db::jsongit::paths::{check_task_folder, find_paths};
use atm_lib::Project;

/// The project at or above `cwd`, on the JSON-in-git store. Nothing is read until the project's data is asked for.
///
/// # Errors
/// When the project's task folder fails its checks (a symlink, or one leading outside the project).
pub fn open_project(cwd: &Path) -> Result<Project> {
    let paths = find_paths(cwd)?;
    // The folder's checks run on every open, so a file linked after the paths were found is refused too
    check_task_folder(&paths.root, &paths.dir)?;
    Ok(Project::new(Box::new(JsonGitStore::new(paths))))
}

/// `atm _lock-test <seconds>`: hold the task folder's lock for `seconds`, logging `acquire <pid>` and `release <pid>`
/// to `.lock-test.log` in it, so tests can check that two processes never hold it at once.
///
/// # Errors
/// When the lock can't be taken in time, or the log can't be written.
pub fn hold_lock_for_test(cwd: &Path, seconds: f64) -> Result<()> {
    use std::io::Write;
    let dir = find_paths(cwd)?.dir;
    let log = dir.join(".lock-test.log");
    let append = |line: String| -> Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)
            .map_err(|e| atm_common::Error::failed(format!("{}: {e}", log.display())))?;
        writeln!(file, "{line}")
            .map_err(|e| atm_common::Error::failed(format!("{}: {e}", log.display())))
    };
    let held = atm_db::jsongit::lock::lock(&dir, None)?;
    append(format!("acquire {}", std::process::id()))?;
    std::thread::sleep(std::time::Duration::from_secs_f64(seconds.max(0.0)));
    append(format!("release {}", std::process::id()))?;
    drop(held);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::open_project;

    #[test]
    fn a_folder_without_a_project_opens_and_says_so_when_read() {
        let project = open_project(&std::env::temp_dir()).unwrap();
        let error = project.store().list_tasks().unwrap_err();
        assert!(
            error.to_string().starts_with("no tasks.json in "),
            "{error}"
        );
    }
}
