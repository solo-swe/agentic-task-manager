//! Watching for changes (T-456, T-507): a watcher on the folders a workspace or a project reports
//! ([`crate::Workspace::watch_folders`], [`crate::workspaces::Member::watch_folders`]; not their subfolders), its events
//! debounced, so a front end hears once per burst of changes that something there changed and reads it again. The
//! app's subscriptions and `atm serve`'s change streams both watch with it.

use std::path::PathBuf;
use std::time::Duration;

use crate::{Workspace, Workspaces};
use atm_common::{Error, Result};
use notify::RecursiveMode;
use notify_debouncer_full::{DebounceEventResult, Debouncer, RecommendedCache, new_debouncer};

/// How long a watch waits for a burst of changes (an atm command writes a file or two, git many) to settle.
pub const SETTLE: Duration = Duration::from_millis(300);

/// The folders to watch for workspace `workspace` of `workspaces` to change: its list's and its projects' (as
/// [`Workspace::watch_folders`] has them), and the state folder, where the workspaces themselves are.
#[must_use]
pub fn workspace_folders(workspaces: &Workspaces, workspace: &Workspace) -> Vec<PathBuf> {
    let mut folders = workspace.watch_folders();
    folders.push(workspaces.folder());
    folders
}

/// A watch on some folders: it watches until it is dropped.
pub struct Watch {
    _debouncer: Debouncer<notify::RecommendedWatcher, RecommendedCache>,
}

/// Watch `folders` (those that are there; one that isn't is skipped), calling `changed` once each burst of changes
/// in them has settled ([`SETTLE`]), on the watcher's own thread.
///
/// # Errors
/// The system's watcher can't be started, or a folder can't be watched.
pub fn watch(folders: &[PathBuf], mut changed: impl FnMut() + Send + 'static) -> Result<Watch> {
    let failed = |e: notify::Error| Error::failed(format!("can't watch for changes: {e}"));
    let mut debouncer = new_debouncer(SETTLE, None, move |result: DebounceEventResult| {
        if result.is_ok_and(|events| !events.is_empty()) {
            changed();
        }
    })
    .map_err(failed)?;
    for folder in folders.iter().filter(|f| f.is_dir()) {
        debouncer
            .watch(folder, RecursiveMode::NonRecursive)
            .map_err(failed)?;
    }
    Ok(Watch {
        _debouncer: debouncer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn a_change_in_a_watched_folder_is_told() {
        let dir = std::env::temp_dir().join(format!("atm-lib-watch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a folder");
        let (sent, received) = mpsc::channel();
        // A folder that isn't there is skipped, not an error
        let watching = watch(&[dir.clone(), dir.join("not-there")], move || {
            let _ = sent.send(());
        })
        .expect("watching");
        std::thread::sleep(Duration::from_millis(200));
        std::fs::write(dir.join("tasks.json"), "{}").expect("a change");
        received
            .recv_timeout(Duration::from_secs(10))
            .expect("told of the change");
        drop(watching);
        std::fs::remove_dir_all(&dir).expect("clean up");
    }
}
